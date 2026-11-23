//! 长截图（需求 3.9、M9.1）。
//!
//! 进入长截图后：
//! - 遮罩进入「透出真实画面」模式（见 [`crate::live`]）：选区里透出真实页面、截屏抓不到遮罩、
//!   滚轮和点击落到下面的页面上，只有长截图工具条接收鼠标；键盘焦点留在遮罩上；
//! - 后台线程每 ~40 ms 抓一次选区，交给 [`Stitcher`] 拼接，长度变化或对不上时通知前端；
//! - 「自动滚动」给选区下面的窗口一格一格地发滚轮消息（不动用户的指针），等画面停稳再滚下一格，
//!   连续几格都不动就是到底了。
//!
//! 完成（复制 / 贴图 / 保存 / 识别文字）时停掉线程、生成长图、结束截图。

use snapmind_core::i18n::tr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use snapmind_core::export::{self, ImageFormat, RgbaImage};
use snapmind_core::stitch::{Axis, Step, Stitcher};
use snapmind_ocr::{layout, Line};
use snapmind_platform::{capture, cursor, Rect};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewWindow};

use crate::export::Region;
use crate::live::{self, Live};
use crate::overlay;
use crate::state::AppState;

/// 进度事件（只发给发起长截图的那个遮罩窗口）。
const EVENT: &str = "long:progress";
/// 抓帧间隔。
const TICK: Duration = Duration::from_millis(40);
/// 长图沿滚动方向最多多少物理像素（30000：够拼 30 屏，也在常见看图软件和剪贴板的承受范围内）。
const MAX_LEN: usize = 30_000;
/// 再大也不超过这么多像素（宽选区时相应缩短），约 480 MB 内存。
const MAX_PIXELS: usize = 120_000_000;

#[derive(Default)]
pub struct LongState {
    inner: Mutex<Option<Running>>,
}

struct Running {
    shared: Arc<Shared>,
    /// 选区左上角（虚拟桌面物理像素）和所在显示器
    origin: (i32, i32),
    mon: overlay::MonitorGeom,
    region: Region,
}

struct Shared {
    stitcher: Mutex<Stitcher>,
    auto: AtomicBool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Progress {
    width: u32,
    height: u32,
    /// ok / lost（对不上）/ full（到上限）/ end（自动滚动到底）
    state: &'static str,
    auto: bool,
    /// 内容版本，预览按它决定要不要刷新
    rev: u64,
}

/// 停止当前的长截图（如果有），退出透出模式。`unpark`：回到普通截图编辑态时把其他屏的遮罩放回去。
fn take<R: Runtime>(app: &AppHandle<R>, unpark: bool) -> Option<Running> {
    let run = app.state::<LongState>().inner.lock().expect("长截图锁").take()?;
    live::exit(app, unpark);
    Some(run)
}

/// 结束截图时调用：有长截图就停掉。
pub fn stop<R: Runtime>(app: &AppHandle<R>) {
    let _ = take(app, false);
}

#[tauri::command]
pub fn long_start(app: AppHandle, window: WebviewWindow, index: usize, region: Region, axis: Axis) -> Result<(), String> {
    let mon = overlay::monitor(&app, index).ok_or_else(|| tr("截图已结束"))?;
    if region.width < 16 || region.height < 16 {
        return Err(tr("选区太小"));
    }
    let _ = take(&app, false); // 切换方向 = 从当前画面重新开始
    let span = match axis {
        Axis::Vertical => region.width,
        Axis::Horizontal => region.height,
    } as usize;
    let max = MAX_LEN.min(MAX_PIXELS / span.max(1));
    let shared = Arc::new(Shared {
        stitcher: Mutex::new(Stitcher::new(axis, region.width, region.height, max)),
        auto: AtomicBool::new(false),
    });
    let origin = (mon.x + region.x as i32, mon.y + region.y as i32);
    let live = Live::enter(&app, &window, (mon.x, mon.y));
    {
        let shared = shared.clone();
        let live = live.clone();
        let rect = Rect { x: origin.0, y: origin.1, width: region.width, height: region.height };
        std::thread::Builder::new()
            .name("long-capture".into())
            .spawn(move || run(window, live, shared, rect, axis))
            .map_err(|e| e.to_string())?;
    }
    *app.state::<LongState>().inner.lock().expect("长截图锁") = Some(Running { shared, origin, mon, region });
    Ok(())
}

/// 后台线程：鼠标穿透切换、抓帧拼接、自动滚动。
fn run(window: WebviewWindow, live: Arc<Live>, shared: Arc<Shared>, rect: Rect, axis: Axis) {
    let horizontal = axis == Axis::Horizontal;
    let along = if horizontal { rect.width } else { rect.height };
    // 每次滚多少：大约选区长度的三分之一（一格 120 通常滚 100 像素左右），选区很短时用三分之一格，
    // 免得一下滚出整个选区；重叠部分留得足，拼接才稳
    let notch = if along < 300 { 40 } else { 120 * (along / 350).clamp(1, 4) as i32 } * if horizontal { 1 } else { -1 };
    let mut rev = 0u64;
    let mut lost = 0u32;
    let mut sent: Option<Progress> = None;
    let mut state: &'static str = "ok";
    // 自动滚动
    let mut was_auto = false;
    let mut quiet = 0u32;
    let mut idle = 0u32;
    let mut moved_since_notch = true;
    let mut last_notch = Instant::now();

    loop {
        let t0 = Instant::now();
        if !live.poll() {
            break;
        }

        let step = match capture::capture_rect(rect) {
            Ok(f) => shared.stitcher.lock().expect("拼接锁").push(f.pixels()),
            Err(_) => Step::Lost,
        };
        match step {
            Step::First | Step::Moved { .. } => {
                rev += 1;
                lost = 0;
                if state == "lost" {
                    state = "ok";
                }
            }
            Step::Same => {
                lost = 0;
                if state == "lost" {
                    state = "ok";
                }
            }
            Step::Lost => {
                lost += 1;
                // 连续几帧对不上才提示（滚动动画中间偶尔一帧对不上很正常）
                if lost >= 4 {
                    state = "lost";
                }
            }
            Step::Full => state = "full",
        }

        // 自动滚动
        let auto = shared.auto.load(Ordering::SeqCst);
        if auto && !was_auto {
            quiet = 0;
            idle = 0;
            moved_since_notch = true;
            if state == "end" {
                state = "ok";
            }
        }
        was_auto = auto;
        if auto {
            match step {
                Step::Moved { .. } => {
                    moved_since_notch = true;
                    quiet = 0;
                }
                Step::Same | Step::First => quiet += 1,
                Step::Lost => quiet = 0,
                Step::Full => {}
            }
            if state == "full" || (state == "lost" && lost >= 12) {
                shared.auto.store(false, Ordering::SeqCst);
            } else if quiet >= 2 && last_notch.elapsed() >= Duration::from_millis(120) {
                idle = if moved_since_notch { 0 } else { idle + 1 };
                if idle >= 3 {
                    shared.auto.store(false, Ordering::SeqCst);
                    state = "end";
                } else {
                    wheel_under(&rect, notch, horizontal);
                    last_notch = Instant::now();
                    moved_since_notch = false;
                    quiet = 0;
                }
            }
        }

        let (width, height) = shared.stitcher.lock().expect("拼接锁").size();
        let p = Progress { width, height, state, auto: shared.auto.load(Ordering::SeqCst), rev };
        if sent.as_ref() != Some(&p) {
            let _ = window.emit_to(window.label(), EVENT, p.clone());
            sent = Some(p);
        }
        if let Some(rest) = TICK.checked_sub(t0.elapsed()) {
            std::thread::sleep(rest);
        }
    }
}

/// 给选区中央下面的窗口（跳过本程序的遮罩）发一次滚轮。
fn wheel_under(rect: &Rect, delta: i32, horizontal: bool) -> bool {
    let (x, y) = (rect.x + rect.width as i32 / 2, rect.y + rect.height as i32 / 2);
    let windows = snapmind_platform::snap::top_level_windows(std::process::id());
    match snapmind_platform::snap::window_at(&windows, x, y) {
        Some(w) => cursor::post_wheel(w.handle, x, y, delta, horizontal),
        None => false,
    }
}

/// 开关自动滚动。
#[tauri::command]
pub fn long_auto(state: State<'_, LongState>, on: bool) {
    if let Some(r) = state.inner.lock().expect("长截图锁").as_ref() {
        r.shared.auto.store(on, Ordering::SeqCst);
    }
}

/// 遮罩自己收到的滚轮（系统设置成「滚轮发给焦点窗口」时，或指针在工具条上时）转给下面的页面。
#[tauri::command]
pub fn long_wheel(state: State<'_, LongState>, delta: i32, horizontal: bool) {
    if let Some(r) = state.inner.lock().expect("长截图锁").as_ref() {
        let rect = Rect { x: r.origin.0, y: r.origin.1, width: r.region.width, height: r.region.height };
        wheel_under(&rect, delta, horizontal);
    }
}

/// 拼接预览：长图缩小到 `thickness` 像素宽（横向时是高），PNG。
#[tauri::command]
pub async fn long_preview(app: AppHandle, thickness: u32) -> Result<tauri::ipc::Response, String> {
    let shared = {
        let st = app.state::<LongState>();
        let g = st.inner.lock().expect("长截图锁");
        g.as_ref().ok_or_else(|| tr("长截图已结束"))?.shared.clone()
    };
    tauri::async_runtime::spawn_blocking(move || {
        let img = {
            let s = shared.stitcher.lock().expect("拼接锁");
            let span = match s.axis() {
                Axis::Vertical => s.size().0,
                Axis::Horizontal => s.size().1,
            };
            s.image(thickness as f64 / span.max(1) as f64)
        };
        if img.width == 0 {
            return Err(tr("还没有画面"));
        }
        export::encode(&img, ImageFormat::Png, 0, true).map(tauri::ipc::Response::new).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 取消长截图，回到普通的截图编辑态。
#[tauri::command]
pub fn long_cancel(app: AppHandle) {
    let _ = take(&app, true);
}

/// 完成长截图要做什么。
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Finish {
    Copy,
    Pin,
    Save,
    Ocr,
}

/// 完成长截图：生成长图并交出去，然后结束截图。`path` 是另存为对话框选的路径（保存时）。
/// 返回：保存的路径 / 复制的临时文件名 / 识别出的文字。
#[tauri::command]
pub async fn long_finish(
    app: AppHandle,
    state: State<'_, AppState>,
    action: Finish,
    path: Option<String>,
) -> Result<String, String> {
    if action == Finish::Ocr && !crate::ocr::installed(&app) {
        return Err(tr("识别文字要用本地 OCR 模型：先退出长截图，按 Shift+C 下载模型"));
    }
    let run = take(&app, false).ok_or_else(|| tr("长截图已结束"))?;
    let img = run.shared.stitcher.lock().expect("拼接锁").image(1.0);
    if img.width == 0 || img.height == 0 {
        return Err(tr("还没有拼接到内容"));
    }
    let config = state.config.get();
    let mon = run.mon;
    let origin = run.origin;
    match action {
        Finish::Copy => {
            overlay::end(&app);
            let template = config.save.filename_template.clone();
            tauri::async_runtime::spawn_blocking(move || crate::export::copy_image(&img, &template))
                .await
                .map_err(|e| e.to_string())?
        }
        Finish::Pin => {
            overlay::end(&app);
            let max = ((mon.width as f64 * 0.9) as u32, (mon.height as f64 * 0.9) as u32);
            let fit = fit_zoom((img.width, img.height), max);
            // 放在选区左上角；缩小后仍放不下时往上挪进屏幕
            let h = (img.height as f64 * fit).round() as i32;
            let y = origin.1.min(mon.y + mon.height as i32 - h - 16).max(mon.y + 8);
            crate::pin::create_zoomed(&app, img, (origin.0, y), mon.scale, fit).map(|id| id.to_string()).map_err(|e| e.to_string())
        }
        Finish::Save => {
            let ext_fmt = config.save.format;
            let target = match path.map(|p| p.trim().to_string()).filter(|p| !p.is_empty()) {
                Some(p) => std::path::PathBuf::from(p),
                None => {
                    let dir = crate::export::default_dir(&app, &config);
                    let stem = export::render_filename(&config.save.filename_template, export::now_local());
                    export::unique_path(&dir, &stem, ext_fmt.extension())
                }
            };
            overlay::end(&app);
            let quality = config.save.quality;
            tauri::async_runtime::spawn_blocking(move || {
                let bytes = export::encode(&img, ext_fmt, quality, false).map_err(|e| e.to_string())?;
                if let Some(dir) = target.parent() {
                    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
                }
                std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
                Ok(target.to_string_lossy().into_owned())
            })
            .await
            .map_err(|e| e.to_string())?
        }
        Finish::Ocr => {
            overlay::end(&app);
            let app2 = app.clone();
            let text = tauri::async_runtime::spawn_blocking(move || recognize_long(&app2, &img))
                .await
                .map_err(|e| e.to_string())??;
            if text.trim().is_empty() {
                return Err(tr("没有识别出文字"));
            }
            snapmind_platform::clipboard::set_text(&text).map_err(|e| e.to_string())?;
            crate::pin::create_text(&app, text.clone(), origin, mon.scale).map_err(|e| e.to_string())?;
            Ok(text)
        }
    }
}

/// 贴图的初始缩放：放得下就 1，否则等比缩小到放得下（不小于贴图允许的最小缩放）。
pub fn fit_zoom(img: (u32, u32), max: (u32, u32)) -> f64 {
    let k = (max.0 as f64 / img.0.max(1) as f64).min(max.1 as f64 / img.1.max(1) as f64);
    k.clamp(crate::pin::ZOOM_MIN, 1.0)
}

/// 沿长边切块：每块 `tile` 长、相邻两块重叠 `overlap`。返回各块的起点和长度。
pub fn tiles(len: u32, tile: u32, overlap: u32) -> Vec<(u32, u32)> {
    if len <= tile {
        return vec![(0, len)];
    }
    let step = tile - overlap;
    let mut out = Vec::new();
    let mut s = 0;
    loop {
        if s + tile >= len {
            out.push((len.saturating_sub(tile), tile.min(len)));
            break;
        }
        out.push((s, tile));
        s += step;
    }
    out
}

/// 一行文字归哪一块：中心落在这一块「自己的」范围里（重叠区一分为二）。
fn owns(tiles: &[(u32, u32)], k: usize, center: u32) -> bool {
    let (s, l) = tiles[k];
    let lo = if k == 0 { 0 } else { (tiles[k - 1].0 + tiles[k - 1].1 + s) / 2 };
    let hi = if k + 1 == tiles.len() { u32::MAX } else { (s + l + tiles[k + 1].0) / 2 };
    center >= lo && center < hi
}

/// 从 RGBA 图里裁一块（调用方保证不越界）。
fn crop(img: &RgbaImage, x: u32, y: u32, w: u32, h: u32) -> RgbaImage {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let s = ((row * img.width + x) * 4) as usize;
        data.extend_from_slice(&img.data[s..s + (w * 4) as usize]);
    }
    RgbaImage { width: w, height: h, data }
}

/// 识别长图里的文字：沿长边切成接近一屏大小的块分别识别（OCR 会把整张图缩到 1440 以内，
/// 长图整张识别字会小到认不出），按中心去掉重叠区里重复的行，再按段落规则拼起来。
fn recognize_long<R: Runtime>(app: &AppHandle<R>, img: &RgbaImage) -> Result<String, String> {
    let vertical = img.height >= img.width;
    let (len, other) = if vertical { (img.height, img.width) } else { (img.width, img.height) };
    let tile = other.clamp(800, 1400).max(160);
    let parts = tiles(len, tile, 120);
    let mut lines: Vec<Line> = Vec::new();
    for (k, &(s, l)) in parts.iter().enumerate() {
        let piece = if vertical { crop(img, 0, s, img.width, l) } else { crop(img, s, 0, l, img.height) };
        for mut line in crate::ocr::recognize(app, &piece)? {
            let center = if vertical { line.y + line.height / 2 } else { line.x + line.width / 2 };
            if !owns(&parts, k, s + center) {
                continue;
            }
            if vertical {
                line.y += s;
                line.ink.top += s;
            } else {
                line.x += s;
                line.ink.left += s;
            }
            lines.push(line);
        }
    }
    Ok(layout::compose(&lines, &(0..lines.len()).collect::<Vec<_>>(), false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tiles_cover_with_overlap() {
        assert_eq!(tiles(500, 1000, 100), vec![(0, 500)]);
        let t = tiles(2500, 1000, 100);
        assert_eq!(t, vec![(0, 1000), (900, 1000), (1500, 1000)]);
        // 每个位置恰好归一块
        for c in 0..2500 {
            assert_eq!((0..t.len()).filter(|&k| owns(&t, k, c)).count(), 1, "center {c}");
        }
    }

    #[test]
    fn fit() {
        assert_eq!(fit_zoom((800, 600), (1700, 900)), 1.0);
        assert!((fit_zoom((800, 9000), (1700, 900)) - 0.1).abs() < 1e-9);
        assert!((fit_zoom((800, 4500), (1700, 900)) - 0.2).abs() < 1e-9);
    }
}
