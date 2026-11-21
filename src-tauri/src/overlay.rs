//! 截图遮罩：每个显示器一个预热好的透明置顶窗口。
//!
//! 流程（编程计划 §4「冻结帧」「窗口预热」）：
//! 1. 启动时为每个显示器建好遮罩窗口，「显示」但停放在屏幕外很远的地方。
//!    不用隐藏窗口：WebView 隐藏几秒后会挂起合成，再显示时第一帧要多等约 140 ms；
//!    停在屏幕外的窗口一直处于可见状态，随时能立刻出图；
//! 2. 按下截图键：先并行抓下所有显示器（遮罩不在屏幕上，不会抓到自己）；
//! 3. 把帧存进内存，通过 `snapframe` 协议提供给前端（BMP，无需编码）；
//! 4. 通知各遮罩窗口新的帧，并把窗口移到对应显示器上。窗口是透明的，帧画出来之前
//!    透出的就是真实桌面，和冻结帧一模一样，遮罩在第一帧就出现；
//! 5. 前端回报 `overlay_ready`（遮罩出现 / 冻结帧画好），记录耗时；
//! 6. 结束截图：前端先清空画面，再把窗口停回屏幕外。

use std::borrow::Cow;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use snapmind_core::export::RgbaImage;
use snapmind_platform::bmp::Frame;
use snapmind_platform::snap::{self, ElementProbe, TopWindow};
use snapmind_platform::{capture, Rect};
use tauri::http::{Request, Response, StatusCode};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

pub const LABEL_PREFIX: &str = "overlay-";
/// 自定义协议名。前端用 `convertFileSrc(index, 'snapframe')` 拿到地址。
pub const PROTOCOL: &str = "snapframe";
/// 通知遮罩窗口开始一次截图。
const EVENT_START: &str = "capture:start";
/// 所有窗口共用的 WebView2 启动参数（同一进程里的 WebView 必须一致，设置窗口在
/// tauri.conf.json 里写了同一串）。前三项是 Tauri 默认值；后面关掉 Chromium 的遮挡检测和
/// 后台降频，否则停在屏幕外的遮罩会被当成「被遮挡」而挂起，出现时慢上百毫秒。
pub const BROWSER_ARGS: &str =
    "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection,CalculateNativeWinOcclusion,UseEcoQoSForBackgroundProcess --disable-backgrounding-occluded-windows --disable-renderer-backgrounding";
/// 停放位置：远离任何显示器（Windows 虚拟桌面坐标上限约 ±32000）。
const PARK: PhysicalPosition<i32> = PhysicalPosition { x: -30000, y: -30000 };

/// 一个显示器的几何信息（物理像素，虚拟桌面坐标）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MonitorGeom {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale: f64,
}

impl MonitorGeom {
    fn rect(&self) -> Rect {
        Rect { x: self.x, y: self.y, width: self.width, height: self.height }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct StartPayload {
    /// 本次截图的序号，前端回报时带上，避免旧回报混进来。
    seq: u64,
    /// 帧在协议里的路径（显示器序号）。
    index: usize,
    monitor: MonitorGeom,
    /// 鼠标是否在这个显示器上（M1.2 起用于决定初始焦点和放大镜）。
    has_cursor: bool,
    /// 截图时的鼠标位置（物理像素），仅在这块屏上时提供
    cursor: Option<CursorPos>,
}

#[derive(Debug, Clone, Copy, Serialize)]
struct CursorPos {
    x: i32,
    y: i32,
}

#[derive(Default)]
pub struct CaptureState {
    seq: AtomicU64,
    inner: Mutex<Session>,
    probe: ElementProbe,
}

/// 吸附链里的一个矩形（物理像素、虚拟桌面坐标）。
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SnapRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl From<Rect> for SnapRect {
    fn from(r: Rect) -> Self {
        Self { x: r.x, y: r.y, width: r.width, height: r.height }
    }
}

#[derive(Default)]
struct Session {
    active: bool,
    /// 按键时刻（Unix 毫秒）。前端回报自己画出遮罩的时刻，两者相减就是真实耗时，
    /// 不受 Rust 主线程当时是否繁忙（例如正在传输截图）影响。
    started: Option<f64>,
    frames: Vec<Option<Arc<Frame>>>,
    /// 截图那一刻的顶层窗口快照（Z 序从上到下），吸附用
    windows: Vec<TopWindow>,
    /// 标注层：(显示器序号, 相对选区的偏移, RGBA)。导出前由前端上传，裁剪后叠到截图上。
    annotations: Option<(usize, (u32, u32), Arc<RgbaImage>)>,
    /// 已回报「遮罩出现」「冻结帧画好」的窗口数
    masks: usize,
    painted: usize,
}

fn epoch_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1e3).unwrap_or(0.0)
}

pub fn label(i: usize) -> String {
    format!("{LABEL_PREFIX}{i}")
}

pub fn is_overlay(label: &str) -> bool {
    label.starts_with(LABEL_PREFIX)
}

fn monitors<R: Runtime>(app: &AppHandle<R>) -> Vec<MonitorGeom> {
    app.available_monitors()
        .unwrap_or_default()
        .into_iter()
        .map(|m| MonitorGeom {
            x: m.position().x,
            y: m.position().y,
            width: m.size().width,
            height: m.size().height,
            scale: m.scale_factor(),
        })
        .collect()
}

fn build_window<R: Runtime>(app: &AppHandle<R>, i: usize) -> tauri::Result<WebviewWindow<R>> {
    let w = WebviewWindowBuilder::new(app, label(i), WebviewUrl::App("overlay/index.html".into()))
        .title("SnapMind")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .focused(false)
        .additional_browser_args(BROWSER_ARGS)
        .build()?;
    let _ = w.set_position(PARK);
    if let Ok(hwnd) = w.hwnd() {
        let h = hwnd.0 as isize;
        snapmind_platform::window::disable_transitions(h);
        snapmind_platform::window::make_tool_window(h);
        snapmind_platform::window::show_no_activate(h);
    }
    Ok(w)
}

/// 让窗口正好盖住显示器。先移位置再改尺寸：跨 DPI 移动时系统会按新显示器重算缩放。
fn place<R: Runtime>(w: &WebviewWindow<R>, m: &MonitorGeom) {
    let size = PhysicalSize::new(m.width, m.height);
    let _ = w.set_position(PhysicalPosition::new(m.x, m.y));
    if w.inner_size().ok() != Some(size) {
        let _ = w.set_size(size);
    }
}

/// 让遮罩窗口与当前显示器一一对应：缺的新建，多的关掉。窗口仍停放在屏幕外。
fn reconcile<R: Runtime>(app: &AppHandle<R>, mons: &[MonitorGeom]) -> Vec<WebviewWindow<R>> {
    let mut out = Vec::with_capacity(mons.len());
    for (i, m) in mons.iter().enumerate() {
        let w = match app.get_webview_window(&label(i)) {
            Some(w) => w,
            None => match build_window(app, i) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("创建遮罩窗口失败：{e}");
                    continue;
                }
            },
        };
        let size = PhysicalSize::new(m.width, m.height);
        if w.inner_size().ok() != Some(size) {
            let _ = w.set_size(size);
        }
        out.push(w);
    }
    let mut extra = mons.len();
    while let Some(w) = app.get_webview_window(&label(extra)) {
        let _ = w.destroy();
        extra += 1;
    }
    out
}

/// 启动时预热：建好所有遮罩窗口并加载页面，第一次截图就不用等。
pub fn prewarm<R: Runtime>(app: &AppHandle<R>) {
    reconcile(app, &monitors(app));
}

/// 开始一次截图（全局快捷键、托盘左键都走这里）。
pub fn begin<R: Runtime>(app: &AppHandle<R>) {
    let t0 = Instant::now();
    let t0_epoch = epoch_ms();
    let state = app.state::<CaptureState>();
    if state.inner.lock().expect("截图状态锁").active {
        return; // 已经在截图中，忽略重复按键
    }

    let mons = monitors(app);
    let rects: Vec<Rect> = mons.iter().map(MonitorGeom::rect).collect();
    // 遮罩停在屏幕外且属于本进程，快照里自然不会有它们
    let windows = snap::top_level_windows(std::process::id());
    let frames: Vec<Option<Arc<Frame>>> = capture::capture_all(&rects)
        .into_iter()
        .map(|r| r.map_err(|e| eprintln!("截屏失败：{e}")).ok().map(Arc::new))
        .collect();
    let t_cap = t0.elapsed();
    if frames.iter().all(Option::is_none) {
        return;
    }

    let seq = state.seq.fetch_add(1, Ordering::SeqCst) + 1;
    {
        let mut s = state.inner.lock().expect("截图状态锁");
        *s = Session { active: true, started: Some(t0_epoch), frames, windows, annotations: None, masks: 0, painted: 0 };
    }

    let cursor = app.cursor_position().ok();
    let windows = reconcile(app, &mons);
    for (i, (w, m)) in windows.iter().zip(&mons).enumerate() {
        let has_cursor = cursor.is_some_and(|c| {
            c.x >= m.x as f64
                && c.y >= m.y as f64
                && c.x < m.x as f64 + m.width as f64
                && c.y < m.y as f64 + m.height as f64
        });
        let cursor_here = cursor.filter(|_| has_cursor).map(|c| CursorPos { x: c.x as i32, y: c.y as i32 });
        let payload = StartPayload { seq, index: i, monitor: *m, has_cursor, cursor: cursor_here };
        let _ = app.emit_to(w.label(), EVENT_START, payload);
        place(w, m);
        if has_cursor {
            // 先走一次 Tauri 的 show，让它记下「可见」，set_focus 才会生效。
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
    eprintln!(
        "[capture #{seq}] 抓屏 {:.1} ms，显示窗口 {:.1} ms（{} 个显示器）",
        t_cap.as_secs_f64() * 1e3,
        t0.elapsed().as_secs_f64() * 1e3,
        mons.len()
    );
}

/// 结束截图：把所有遮罩停回屏幕外，释放帧。
pub fn end<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<CaptureState>();
    *state.inner.lock().expect("截图状态锁") = Session::default();
    for (label, w) in app.webview_windows() {
        if is_overlay(&label) {
            let _ = w.set_position(PARK);
        }
    }
}

/// 前端回报进度。所有显示器都回报某一阶段后记录耗时：
/// `mask` = 按键 → 遮罩出现（性能预算 ≤ 120 ms），`frame` = 按键 → 冻结帧画好。
pub fn ready<R: Runtime>(app: &AppHandle<R>, seq: u64, stage: &str, at: f64) -> Option<f64> {
    let state = app.state::<CaptureState>();
    if state.seq.load(Ordering::SeqCst) != seq {
        return None;
    }
    let mut s = state.inner.lock().expect("截图状态锁");
    let total = s.frames.iter().filter(|f| f.is_some()).count();
    let (count, what) = match stage {
        "mask" => (&mut s.masks, "遮罩出现"),
        _ => (&mut s.painted, "冻结帧画好"),
    };
    *count += 1;
    let all = *count == total;
    let ms = at - s.started?;
    if all {
        eprintln!("[capture #{seq}] 按键 → {what} {ms:.1} ms");
    }
    Some(ms)
}

/// 设置（None 为清除）当前会话的标注层。
pub fn set_annotations<R: Runtime>(app: &AppHandle<R>, index: usize, layer: Option<((u32, u32), RgbaImage)>) {
    let state = app.state::<CaptureState>();
    let mut s = state.inner.lock().expect("截图状态锁");
    if s.active {
        s.annotations = layer.map(|(at, l)| (index, at, Arc::new(l)));
    }
}

/// 某块屏的标注层及其相对选区的偏移（没有标注时返回 None）。
pub fn annotations<R: Runtime>(app: &AppHandle<R>, index: usize) -> Option<((u32, u32), Arc<RgbaImage>)> {
    let state = app.state::<CaptureState>();
    let s = state.inner.lock().expect("截图状态锁");
    s.annotations.as_ref().filter(|(i, _, _)| *i == index).map(|(_, at, l)| (*at, l.clone()))
}

/// 当前会话里某块屏的冻结帧（截图已结束时返回 None）。
pub fn frame<R: Runtime>(app: &AppHandle<R>, index: usize) -> Option<Arc<Frame>> {
    let state = app.state::<CaptureState>();
    let s = state.inner.lock().expect("截图状态锁");
    if !s.active {
        return None;
    }
    s.frames.get(index).cloned().flatten()
}

/// 吸附链：鼠标（物理坐标）下从最内层控件到顶层窗口的一串矩形；桌面空白处返回空。
/// UI Automation 查询是阻塞的，调用方要放到后台线程。
pub fn snap_chain<R: Runtime>(app: &AppHandle<R>, x: i32, y: i32) -> Vec<SnapRect> {
    let state = app.state::<CaptureState>();
    let window = {
        let s = state.inner.lock().expect("截图状态锁");
        if !s.active {
            return Vec::new();
        }
        snap::window_at(&s.windows, x, y).copied()
    };
    let Some(window) = window else { return Vec::new() };
    state
        .probe
        .chain(window, x, y, std::time::Duration::from_millis(150))
        .into_iter()
        .map(SnapRect::from)
        .collect()
}

/// `snapframe` 协议：`/<显示器序号>` 返回该显示器当前截图的 BMP。
pub fn serve<R: Runtime>(app: &AppHandle<R>, req: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let index = req.uri().path().trim_start_matches('/').parse::<usize>().ok();
    let frame = index.and_then(|i| {
        let state = app.state::<CaptureState>();
        let s = state.inner.lock().expect("截图状态锁");
        s.frames.get(i).cloned().flatten()
    });
    let builder = Response::builder()
        .header("Access-Control-Allow-Origin", "*") // 前端画布要读像素（取色、放大镜）
        .header("Cache-Control", "no-store");
    match frame {
        Some(f) => builder
            .header("Content-Type", "image/bmp")
            .body(Cow::Owned(f.bmp.clone()))
            .expect("响应头合法"),
        None => builder
            .status(StatusCode::NOT_FOUND)
            .body(Cow::Borrowed(&[][..]))
            .expect("响应头合法"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_labels() {
        assert_eq!(label(2), "overlay-2");
        assert!(is_overlay("overlay-0"));
        assert!(!is_overlay("settings"));
    }
}
