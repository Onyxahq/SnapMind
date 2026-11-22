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
    pub downloading: bool,
    pub received: u64,
    pub total: u64,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct OcrState {
    engine: Mutex<Option<Arc<Ocr>>>,
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
    st.installed = models::installed(&models_dir(app));
    st.total = models::total_size();
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
    eprintln!("本地 OCR 引擎加载 {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    *slot = Some(e.clone());
    Ok(e)
}

/// 识别一张图，返回按阅读顺序的行（阻塞）。
fn recognize<R: Runtime>(app: &AppHandle<R>, img: &RgbaImage) -> Result<Vec<Line>, String> {
    let ocr = engine(app)?;
    let (lines, t) = ocr
        .recognize(Image { width: img.width, height: img.height, rgba: &img.data })
        .map_err(|e| e.to_string())?;
    eprintln!("本地 OCR：{}×{}，{} 行，{:.0} ms", img.width, img.height, lines.len(), t.total.as_secs_f64() * 1e3);
    Ok(lines)
}

/// 识别链里的「本地 OCR」：识别并按段落拼成文字（阻塞）。
pub fn recognize_text<R: Runtime>(app: &AppHandle<R>, img: &RgbaImage) -> Result<String, String> {
    let lines = recognize(app, img)?;
    Ok(layout::compose(&lines, &(0..lines.len()).collect::<Vec<_>>(), false))
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

/// 卡片关掉后释放识别结果。
#[tauri::command]
pub fn ocr_forget(app: AppHandle, id: String) {
    app.state::<OcrState>().results.lock().expect("OCR 结果锁").retain(|(k, _)| *k != id);
}

#[tauri::command]
pub fn ocr_status(app: AppHandle) -> OcrStatus {
    status(&app)
}

/// 下载模型。已经在下载时直接返回；进度和结果都通过 `ocr:download` 事件广播。
#[tauri::command]
pub async fn ocr_download(app: AppHandle) -> Result<(), String> {
    {
        let state = app.state::<OcrState>();
        let mut d = state.download.lock().expect("OCR 状态锁");
        if d.downloading {
            return Ok(());
        }
        *d = OcrStatus { downloading: true, ..Default::default() };
    }
    let _ = app.emit(DOWNLOAD_EVENT, status(&app));
    let r = download(&app).await;
    {
        let state = app.state::<OcrState>();
        let mut d = state.download.lock().expect("OCR 状态锁");
        d.downloading = false;
        d.error = r.as_ref().err().cloned();
    }
    if let Err(e) = &r {
        eprintln!("下载 OCR 模型失败：{e}");
    }
    let _ = app.emit(DOWNLOAD_EVENT, status(&app));
    r
}

fn set_received<R: Runtime>(app: &AppHandle<R>, received: u64) {
    app.state::<OcrState>().download.lock().expect("OCR 状态锁").received = received;
    let _ = app.emit(DOWNLOAD_EVENT, status(app));
}

async fn download<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let dir = user_models_dir(app);
    std::fs::create_dir_all(&dir).map_err(|e| format!("建立模型目录失败：{e}"))?;
    let client = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| e.to_string())?;
    let mut received = 0u64;
    for f in &models::FILES {
        let path = dir.join(f.name);
        if std::fs::read(&path).is_ok_and(|b| hex(&Sha256::digest(&b)) == f.sha256) {
            received += f.size;
            set_received(app, received);
            continue;
        }
        let part = dir.join(format!("{}.part", f.name));
        let r = fetch(app, &client, f, &part, received).await;
        if r.is_err() {
            let _ = std::fs::remove_file(&part);
        }
        r?;
        std::fs::rename(&part, &path).map_err(|e| format!("保存 {} 失败：{e}", f.name))?;
        received += f.size;
        set_received(app, received);
    }
    let yml = std::fs::read_to_string(dir.join("rec.yml")).map_err(|e| e.to_string())?;
    let keys = models::extract_keys(&yml)?;
    std::fs::write(dir.join("keys.txt"), keys.join("\n")).map_err(|e| format!("写字符表失败：{e}"))?;
    Ok(())
}

/// 下载一个文件到 `part`，边收边算哈希，不符就报错。
async fn fetch<R: Runtime>(
    app: &AppHandle<R>,
    client: &reqwest::Client,
    f: &models::ModelFile,
    part: &Path,
    base: u64,
) -> Result<(), String> {
    use std::io::Write;
    let net = |e: reqwest::Error| format!("下载 {} 失败：{e}", f.name);
    let mut resp = client.get(f.url).send().await.map_err(net)?;
    if !resp.status().is_success() {
        return Err(format!("下载 {} 失败：HTTP {}", f.name, resp.status().as_u16()));
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
        return Err(format!("{} 校验失败（SHA-256 不符），可能下载被篡改或不完整", f.name));
    }
    Ok(())
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}
