//! 本地 OCR 接入应用（M4.4）：模型下载、引擎加载、截图识别、按选中的行重新拼接。
//!
//! - 模型不随安装包发布（安装包 ≤ 25 MB 的预算），第一次用时在卡片里点「下载」，从 PaddlePaddle 官方仓库
//!   下载到 `配置目录/models/ppocrv5`，逐个校验 SHA-256；进度通过 `ocr:download` 事件广播。
//! - 引擎第一次识别时加载，之后常驻（加载一次约 0.3 s，常驻约 100 MB 内存）。
//! - 识别结果按任务 id 留几份在 Rust 这边，右键划选只传行号回来，由 `layout::compose` 按段落规则重新拼。

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use serde::Serialize;
use sha2::{Digest, Sha256};
use snapmind_core::export::RgbaImage;
use snapmind_ocr::table::{Table, TableModel};
use snapmind_ocr::{layout, models, Image, Line, Ocr};
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::export::Region;

const DOWNLOAD_EVENT: &str = "ocr:download";
/// 留几份识别结果给右键划选用（每张卡片一份，卡片关掉就没用了）
const KEEP_RESULTS: usize = 4;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrStatus {
    pub installed: bool,
    /// 表格模型（M8.2）也装好了
    pub table_installed: bool,
    pub downloading: bool,
    pub received: u64,
    pub total: u64,
    pub error: Option<String>,
    /// 正在用（或最后用）哪个源下载
    pub source: Option<String>,
}

#[derive(Default)]
pub struct OcrState {
    engine: Mutex<Option<Arc<Ocr>>>,
    table: Mutex<Option<Arc<TableModel>>>,
    download: Mutex<OcrStatus>,
    results: Mutex<Vec<(String, Arc<Vec<Line>>)>>,
}

/// 模型下载到哪里：`配置目录/models/ppocrv5`，环境变量 SNAPMIND_MODELS 可以指定别处。
fn user_models_dir<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    if let Some(p) = std::env::var_os("SNAPMIND_MODELS") {
        return p.into();
    }
    crate::state::config_dir(app).map(|d| d.join("models").join("ppocrv5")).unwrap_or_else(|_| PathBuf::from("models"))
}

/// 引擎从哪里加载：下载目录；调试版在那里没有模型时用仓库里的（`node scripts/fetch-models.mjs`）。
pub fn models_dir<R: Runtime>(app: &AppHandle<R>) -> PathBuf {
    let user = user_models_dir(app);
    if cfg!(debug_assertions) && !models::installed(&user) {
        let dev = Path::new(env!("CARGO_MANIFEST_DIR")).join("../models/ppocrv5");
        if models::installed(&dev) {
            return dev;
        }
    }
    user
}

pub fn status<R: Runtime>(app: &AppHandle<R>) -> OcrStatus {
    let s = app.state::<OcrState>();
    let mut st = s.download.lock().expect("OCR 状态锁").clone();
    let dir = models_dir(app);
    st.installed = models::installed(&dir);
    st.table_installed = models::table_installed(&dir);
    if !st.downloading {
        st.total = models::total_size();
    }
    st
}

/// 加载好的引擎（第一次调用时加载，阻塞，要在后台线程里调）。
fn engine<R: Runtime>(app: &AppHandle<R>) -> Result<Arc<Ocr>, String> {
    let state = app.state::<OcrState>();
    let mut slot = state.engine.lock().expect("OCR 引擎锁");
    if let Some(e) = slot.as_ref() {
        return Ok(e.clone());
    }
    let dir = models_dir(app);
    if !models::installed(&dir) {
        return Err("本地 OCR 模型还没有下载".into());
    }
    let t = Instant::now();
    let e = Arc::new(Ocr::load(&dir).map_err(|e| e.to_string())?);
    crate::log_info!("本地 OCR 引擎加载 {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    *slot = Some(e.clone());
    Ok(e)
}

/// 模型已经下载好（引擎可以加载）。
pub fn installed<R: Runtime>(app: &AppHandle<R>) -> bool {
    models::installed(&models_dir(app))
}

/// 识别一张图，返回按阅读顺序的行（阻塞）。
pub fn recognize<R: Runtime>(app: &AppHandle<R>, img: &RgbaImage) -> Result<Vec<Line>, String> {
    let ocr = engine(app)?;
    let (lines, t) = ocr
        .recognize(Image { width: img.width, height: img.height, rgba: &img.data })
        .map_err(|e| e.to_string())?;
    crate::log_info!("本地 OCR：{}×{}，{} 行，{:.0} ms", img.width, img.height, lines.len(), t.total.as_secs_f64() * 1e3);
    Ok(lines)
}

/// 识别链里的「本地 OCR」：识别并按段落拼成文字（阻塞）。
pub fn recognize_text<R: Runtime>(app: &AppHandle<R>, img: &RgbaImage) -> Result<String, String> {
    let lines = recognize(app, img)?;
    Ok(layout::compose(&lines, &(0..lines.len()).collect::<Vec<_>>(), false))
}

/// 加载好的表格模型（第一次用时加载，阻塞）。
fn table_model<R: Runtime>(app: &AppHandle<R>) -> Result<Arc<TableModel>, String> {
    let state = app.state::<OcrState>();
    let mut slot = state.table.lock().expect("表格模型锁");
    if let Some(m) = slot.as_ref() {
        return Ok(m.clone());
    }
    let dir = models_dir(app);
    if !models::table_installed(&dir) {
        return Err("表格模型还没有下载".into());
    }
    let m = Arc::new(TableModel::load(&dir.join("table.onnx")).map_err(|e| e.to_string())?);
    *slot = Some(m.clone());
    Ok(m)
}

/// 本地表格识别：SLANet 还原结构 + OCR 填字（阻塞）。
pub fn recognize_table<R: Runtime>(app: &AppHandle<R>, img: &RgbaImage) -> Result<Table, String> {
    let model = table_model(app)?;
    let image = Image { width: img.width, height: img.height, rgba: &img.data };
    let t = Instant::now();
    let s = model.structure(image).map_err(|e| e.to_string())?;
    let lines = recognize(app, img)?;
    crate::log_info!("表格结构 {:.0} ms，{} 个单元格", t.elapsed().as_secs_f64() * 1e3, s.cells.len());
    Ok(snapmind_ocr::table::assemble(&s, &lines))
}

/// 本地表格识别要用的模型都在（识别 + 表格）。
pub fn table_ready<R: Runtime>(app: &AppHandle<R>) -> bool {
    let dir = models_dir(app);
    models::installed(&dir) && models::table_installed(&dir)
}

/// 识别链里的「本地 OCR」做 Markdown：按版面转换（阻塞）。
pub fn recognize_markdown<R: Runtime>(app: &AppHandle<R>, img: &RgbaImage) -> Result<String, String> {
    Ok(snapmind_ocr::markdown::from_lines(&recognize(app, img)?))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLineOut {
    /// 相对选区左上角的像素坐标（截图物理像素）
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrResult {
    lines: Vec<OcrLineOut>,
    /// 段落（行下标，按阅读顺序）
    paragraphs: Vec<Vec<usize>>,
    /// 按段落拼好的全文
    text: String,
    ms: u64,
}

/// 识别截图选区（原图，不带标注）。结果按 `id` 留着，供 `ocr_compose` 按选中的行重新拼接。
#[tauri::command]
pub async fn capture_ocr(app: AppHandle, id: String, index: usize, region: Region) -> Result<OcrResult, String> {
    let img = crate::export::crop_base(&app, index, region)?;
    let t0 = Instant::now();
    let handle = app.clone();
    let lines = tauri::async_runtime::spawn_blocking(move || recognize(&handle, &img)).await.map_err(|e| e.to_string())??;
    let paragraphs = layout::paragraphs(&lines);
    let text = paragraphs.iter().map(|p| layout::paragraph_text(&lines, p)).collect::<Vec<_>>().join("\n\n");
    let out = OcrResult {
        lines: lines
            .iter()
            .map(|l| OcrLineOut { x: l.x, y: l.y, width: l.width, height: l.height, text: l.text.clone() })
            .collect(),
        paragraphs,
        text,
        ms: t0.elapsed().as_millis() as u64,
    };
    let state = app.state::<OcrState>();
    let mut kept = state.results.lock().expect("OCR 结果锁");
    kept.retain(|(k, _)| *k != id);
    kept.push((id, Arc::new(lines)));
    let excess = kept.len().saturating_sub(KEEP_RESULTS);
    kept.drain(..excess);
    Ok(out)
}

/// 按选中的行（为空表示全部）重新分段拼接；`by_line` 时一行一行。
#[tauri::command]
pub fn ocr_compose(app: AppHandle, id: String, pick: Option<Vec<usize>>, by_line: bool) -> Result<String, String> {
    let state = app.state::<OcrState>();
    let lines = state
        .results
        .lock()
        .expect("OCR 结果锁")
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, l)| l.clone())
        .ok_or("识别结果已过期")?;
    let pick = pick.unwrap_or_else(|| (0..lines.len()).collect());
    Ok(layout::compose(&lines, &pick, by_line))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinOcrLine {
    /// 笔画的范围（图片像素），透明文字层按它摆放和拉伸
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinOcr {
    lines: Vec<PinOcrLine>,
    paragraphs: Vec<Vec<usize>>,
}

/// 贴图的自动识别（需求第 6 节、M4.5）：贴出后页面调用，在后台识别原图（不带标注）。
/// 文字卡片（原文已知）和还没下载模型时返回 None——不替用户下载，模型在识别卡片里下载。
#[tauri::command]
pub async fn pin_ocr(app: AppHandle, window: tauri::WebviewWindow) -> Result<Option<PinOcr>, String> {
    let Some(id) = crate::pin::id_of(window.label()) else { return Ok(None) };
    let Some((img, is_text)) = crate::pin::base_image(&app, id) else { return Ok(None) };
    if is_text || !installed(&app) {
        return Ok(None);
    }
    let handle = app.clone();
    let lines = tauri::async_runtime::spawn_blocking(move || recognize(&handle, &img)).await.map_err(|e| e.to_string())??;
    let paragraphs = layout::paragraphs(&lines);
    let out = PinOcr {
        lines: lines
            .iter()
            .map(|l| PinOcrLine { x: l.ink.left, y: l.ink.top, width: l.ink.width, height: l.ink.height, text: l.text.clone() })
            .collect(),
        paragraphs,
    };
    crate::pin::set_ocr(&app, id, Arc::new(lines));
    Ok(Some(out))
}

/// 复制贴图上拖选的文字：`pieces` 是选中的行号和选中的那部分文字（首尾两行可能只选了一半），
/// 按段落规则拼接后写进剪贴板，返回拼好的文字。
#[tauri::command]
pub fn pin_copy_text(app: AppHandle, window: tauri::WebviewWindow, pieces: Vec<(usize, String)>) -> Result<String, String> {
    let id = crate::pin::id_of(window.label()).ok_or("不是贴图窗口")?;
    let lines = crate::pin::ocr(&app, id).ok_or("贴图还没有识别结果")?;
    let text = compose_pieces(&lines, &pieces);
    if text.is_empty() {
        return Err("没有选中文字".into());
    }
    snapmind_platform::clipboard::set_text(&text).map_err(|e| e.to_string())?;
    Ok(text)
}

/// 用选中的部分文字替换对应行的文字，再按段落规则拼接。
pub fn compose_pieces(lines: &[Line], pieces: &[(usize, String)]) -> String {
    let mut picked: Vec<Line> = Vec::new();
    for (i, t) in pieces {
        if let Some(l) = lines.get(*i) {
            if !t.trim().is_empty() {
                picked.push(Line { text: t.clone(), ..l.clone() });
            }
        }
    }
    layout::compose(&picked, &(0..picked.len()).collect::<Vec<_>>(), false)
}

/// 卡片关掉后释放识别结果。
#[tauri::command]
pub fn ocr_forget(app: AppHandle, id: String) {
    app.state::<OcrState>().results.lock().expect("OCR 结果锁").retain(|(k, _)| *k != id);
}

#[tauri::command]
pub fn ocr_status(app: AppHandle) -> OcrStatus {
    status(&app)
}

/// 下载模型。`table` 为 true 时连表格模型一起下（已经下好、校验通过的文件跳过）。
/// 已经在下载时直接返回；进度和结果都通过 `ocr:download` 事件广播。
#[tauri::command]
pub async fn ocr_download(app: AppHandle, table: Option<bool>) -> Result<(), String> {
    let mut files: Vec<&'static models::ModelFile> = models::FILES.iter().collect();
    if table == Some(true) {
        files.extend(models::TABLE_FILES.iter());
    }
    {
        let state = app.state::<OcrState>();
        let mut d = state.download.lock().expect("OCR 状态锁");
        if d.downloading {
            return Ok(());
        }
        *d = OcrStatus { downloading: true, total: files.iter().map(|f| f.size).sum(), ..Default::default() };
    }
    let _ = app.emit(DOWNLOAD_EVENT, status(&app));
    let r = download(&app, &files).await;
    {
        let state = app.state::<OcrState>();
        let mut d = state.download.lock().expect("OCR 状态锁");
        d.downloading = false;
        d.error = r.as_ref().err().cloned();
    }
    if let Err(e) = &r {
        crate::log_warn!("下载 OCR 模型失败：{e}");
    }
    let _ = app.emit(DOWNLOAD_EVENT, status(&app));
    r
}

fn set_received<R: Runtime>(app: &AppHandle<R>, received: u64) {
    app.state::<OcrState>().download.lock().expect("OCR 状态锁").received = received;
    let _ = app.emit(DOWNLOAD_EVENT, status(app));
}

async fn download<R: Runtime>(app: &AppHandle<R>, files: &[&models::ModelFile]) -> Result<(), String> {
    let dir = user_models_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("建立模型目录失败：{e}"))?;
    let client = reqwest::Client::builder()
        // 魔搭对不带 User-Agent 的请求回 403
        .user_agent(concat!("SnapMind/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        // 连上了但半天不来数据（被墙时常见）也算失败，好换下一个源
        .read_timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| e.to_string())?;
    // 按地区排先后；某个源成功过就一直先用它
    let mut order = models::sources(snapmind_platform::region::mainland_china()).to_vec();
    let mut received = 0u64;
    for &f in files {
        let path = dir.join(f.name);
        if std::fs::read(&path).is_ok_and(|b| hex(&Sha256::digest(&b)) == f.sha256) {
            received += f.size;
            set_received(app, received);
            continue;
        }
        let part = dir.join(format!("{}.part", f.name));
        let mut errors = Vec::new();
        for i in 0..order.len() {
            let src = order[i];
            set_source(app, src);
            match fetch(app, &client, f, src, &part, received).await {
                Ok(()) => {
                    order.swap(0, i);
                    errors.clear();
                    break;
                }
                Err(e) => {
                    let _ = std::fs::remove_file(&part);
                    crate::log_warn!("从 {} 下载 {} 失败：{e}", src.label(), f.name);
                    set_received(app, received);
                    errors.push(format!("{}：{e}", src.label()));
                }
            }
        }
        if !errors.is_empty() {
            return Err(format!("下载 {} 失败（{}）", f.name, errors.join("；")));
        }
        std::fs::rename(&part, &path).map_err(|e| format!("保存 {} 失败：{e}", f.name))?;
        received += f.size;
        set_received(app, received);
    }
    let yml = std::fs::read_to_string(dir.join("rec.yml")).map_err(|e| e.to_string())?;
    let keys = models::extract_keys(&yml)?;
    std::fs::write(dir.join("keys.txt"), keys.join("\n")).map_err(|e| format!("写字符表失败：{e}"))?;
    Ok(())
}

fn set_source<R: Runtime>(app: &AppHandle<R>, src: models::Source) {
    app.state::<OcrState>().download.lock().expect("OCR 状态锁").source = Some(src.label().into());
    let _ = app.emit(DOWNLOAD_EVENT, status(app));
}

/// 从 `src` 下载一个文件到 `part`，边收边算哈希，不符就报错。
async fn fetch<R: Runtime>(
    app: &AppHandle<R>,
    client: &reqwest::Client,
    f: &models::ModelFile,
    src: models::Source,
    part: &Path,
    base: u64,
) -> Result<(), String> {
    use std::io::Write;
    let net = |e: reqwest::Error| if e.is_timeout() { "连接超时".to_string() } else { e.to_string() };
    let mut resp = client.get(f.url(src)).send().await.map_err(net)?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {}", resp.status().as_u16()));
    }
    let mut file = std::fs::File::create(part).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let (mut got, mut shown) = (0u64, 0u64);
    while let Some(chunk) = resp.chunk().await.map_err(net)? {
        hasher.update(&chunk);
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        got += chunk.len() as u64;
        // 每 256 KB 报一次进度
        if got - shown >= 256 * 1024 {
            shown = got;
            set_received(app, base + got.min(f.size));
        }
    }
    file.flush().map_err(|e| e.to_string())?;
    let sum = hex(&hasher.finalize());
    if sum != f.sha256 {
        return Err("校验失败（SHA-256 不符），可能下载被篡改或不完整".into());
    }
    Ok(())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use snapmind_ocr::Ink;

    fn line(text: &str, y: u32) -> Line {
        let ink = Ink { left: 10, top: y, width: 300, height: 14, color: [0; 3], background: [255; 3] };
        Line { text: text.into(), x: 10, y, width: 300, height: 14, score: 1.0, ink }
    }

    #[test]
    fn partial_selection_is_rejoined_by_paragraph_rules() {
        let lines = vec![line("The first line of a para-", 10), line("graph keeps going here.", 30), line("新的段落", 80)];
        // 第一行只选了后半段，第二行整行，第三行前两个字
        let pieces = vec![(0, "line of a para-".to_string()), (1, "graph keeps going here.".to_string()), (2, "新的".to_string())];
        assert_eq!(compose_pieces(&lines, &pieces), "line of a paragraph keeps going here.\n\n新的");
        assert_eq!(compose_pieces(&lines, &[(9, "x".into()), (1, "  ".into())]), "");
    }

    #[test]
    fn hex_digest() {
        assert_eq!(hex(&Sha256::digest(b"abc"))[..16], *"ba7816bf8f01cfea");
    }
}
