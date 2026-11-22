//! 悬浮球（需求第 11 节、M10.5，可选，默认关闭）：屏幕边缘的快捷入口。
//!
//! - 单击开始截图；右键弹出菜单（截图、贴图、设置、隐藏悬浮球）。
//! - 拖动用系统自己的窗口拖动（页面里按下后移动超过几像素就调 startDragging），完全跟手；
//!   松手后贴到离得近的左 / 右边，一半藏在屏幕外；指针移上去时滑出来，移开后再缩回去。
//! - 截屏抓不到它（WDA_EXCLUDEFROMCAPTURE），不会出现在截图和录屏里。
//! - 位置记在配置里（窗口中心），下次按这个点重新找显示器贴靠。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::json;
use snapmind_core::config::BallConfig;
use snapmind_platform::cursor;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::{AppHandle, Manager, PhysicalPosition, Runtime, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

pub const LABEL: &str = "ball";
/// 窗口边长（CSS 像素）：球 44 + 四周留给阴影。页面里同一个值。
const SIZE_CSS: f64 = 56.0;
/// 露出来时离屏幕边多远（物理像素按缩放换算前的 CSS 像素）
const GAP_CSS: f64 = 6.0;

#[derive(Default)]
pub struct BallState {
    /// 程序自己挪窗口（贴边、滑出、刚显示时摆位置）后的一小段时间里，Moved 事件不算用户拖动
    quiet_until: Mutex<Option<Instant>>,
    /// 最后一次被用户拖动的时间；有值时后台线程等拖动结束再贴边
    dragged_at: Mutex<Option<Instant>>,
    /// 当前是滑出状态
    shown: AtomicBool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

/// 显示器工作区（物理像素）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Work {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// 贴边后的窗口左上角：离哪边近贴哪边，纵向夹在工作区里；`shown` 为 false 时一半藏到屏幕外。
pub fn place(work: Work, size: i32, gap: i32, center: (i32, i32), shown: bool) -> (i32, i32, Side) {
    let side = if center.0 < work.x + work.w / 2 { Side::Left } else { Side::Right };
    let y = (center.1 - size / 2).clamp(work.y + gap, (work.y + work.h - size - gap).max(work.y + gap));
    let x = match (side, shown) {
        (Side::Left, false) => work.x - size / 2,
        (Side::Left, true) => work.x + gap,
        (Side::Right, false) => work.x + work.w - size / 2,
        (Side::Right, true) => work.x + work.w - size - gap,
    };
    (x, y, side)
}

/// 包含某点的显示器的工作区和缩放（找不到时用主显示器）。
fn monitor_at<R: Runtime>(app: &AppHandle<R>, p: (i32, i32)) -> Option<(Work, f64)> {
    let mons = app.available_monitors().ok()?;
    let inside = |m: &tauri::Monitor| {
        let (pos, s) = (m.position(), m.size());
        p.0 >= pos.x && p.1 >= pos.y && p.0 < pos.x + s.width as i32 && p.1 < pos.y + s.height as i32
    };
    let m = mons.iter().find(|m| inside(m)).cloned().or_else(|| app.primary_monitor().ok().flatten()).or(mons.first().cloned())?;
    let wa = m.work_area();
    Some((Work { x: wa.position.x, y: wa.position.y, w: wa.size.width as i32, h: wa.size.height as i32 }, m.scale_factor()))
}

/// 默认位置：主显示器右边缘、约 2/3 高处。
fn default_center<R: Runtime>(app: &AppHandle<R>) -> (i32, i32) {
    match app.primary_monitor().ok().flatten() {
        Some(m) => {
            let wa = m.work_area();
            (wa.position.x + wa.size.width as i32 - 10, wa.position.y + wa.size.height as i32 * 2 / 3)
        }
        None => (1900, 700),
    }
}

/// 按配置显示或关掉悬浮球（启动时和配置变化时调用）。
pub fn sync<R: Runtime>(app: &AppHandle<R>, cfg: &BallConfig) {
    let existing = app.get_webview_window(LABEL);
    match (cfg.enabled, existing) {
        (true, None) => {
            if let Err(e) = create(app, cfg) {
                eprintln!("创建悬浮球失败：{e}");
            }
        }
        (false, Some(w)) => {
            let _ = w.destroy();
        }
        _ => {}
    }
}

fn create<R: Runtime>(app: &AppHandle<R>, cfg: &BallConfig) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("ball/index.html".into()))
        .title("SnapMind 悬浮球")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .visible(false)
        .inner_size(SIZE_CSS, SIZE_CSS)
        .additional_browser_args(crate::overlay::BROWSER_ARGS)
        .build()?;
    if let Ok(h) = w.hwnd() {
        let h = h.0 as isize;
        snapmind_platform::window::disable_transitions(h);
        snapmind_platform::window::make_tool_window(h);
        snapmind_platform::window::make_popup(h);
        snapmind_platform::window::exclude_from_capture(h, true);
    }
    let center = cfg.pos.map(|p| (p[0], p[1])).unwrap_or_else(|| default_center(app));
    let spot = monitor_at(app, center).map(|(work, scale)| {
        let size = (SIZE_CSS * scale).round() as i32;
        let (x, y, _) = place(work, size, (GAP_CSS * scale).round() as i32, center, false);
        (size, x, y)
    });
    app.state::<BallState>().quiet_for(1500);
    if let Some((size, x, y)) = spot {
        let _ = w.set_size(tauri::PhysicalSize::new(size as u32, size as u32));
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    let app2 = app.clone();
    w.on_menu_event(move |_, e| match e.id().as_ref() {
        "ball-capture" => crate::actions::capture(&app2),
        "ball-pin" => crate::actions::pin_clipboard(&app2),
        "ball-settings" => crate::wm::show_settings(&app2, None),
        "ball-hide" => {
            if let Err(e) = crate::configfile::update(&app2, &json!({ "ball": { "enabled": false } })) {
                eprintln!("关闭悬浮球失败：{e}");
            }
        }
        _ => {}
    });
    let _ = w.show();
    // 显示时系统会把一半在屏幕外的窗口挪回来：显示之后再放一次
    if let Some((_, x, y)) = spot {
        let _ = w.set_position(PhysicalPosition::new(x, y));
    }
    app.state::<BallState>().shown.store(false, Ordering::SeqCst);
    Ok(())
}

impl BallState {
    /// 接下来 `ms` 毫秒内的 Moved 事件都是程序自己挪的。
    fn quiet_for(&self, ms: u64) {
        *self.quiet_until.lock().expect("悬浮球锁") = Some(Instant::now() + Duration::from_millis(ms));
    }
    fn is_quiet(&self) -> bool {
        self.quiet_until.lock().expect("悬浮球锁").is_some_and(|t| Instant::now() < t)
    }
}

/// 动画移到目标位置（约 140 ms，先快后慢）。
fn animate<R: Runtime>(w: &WebviewWindow<R>, state: &BallState, to: (i32, i32)) {
    let Ok(from) = w.outer_position() else { return };
    state.quiet_for(400);
    // 减弱动效：直接到位
    let reduced = w.app_handle().state::<crate::state::AppState>().config.get().appearance.motion == snapmind_core::config::Motion::Reduced;
    if reduced {
        let _ = w.set_position(PhysicalPosition::new(to.0, to.1));
        state.quiet_for(200);
        return;
    }
    const STEPS: i32 = 9;
    for i in 1..=STEPS {
        let t = i as f64 / STEPS as f64;
        let k = 1.0 - (1.0 - t).powi(3);
        let x = from.x + ((to.0 - from.x) as f64 * k).round() as i32;
        let y = from.y + ((to.1 - from.y) as f64 * k).round() as i32;
        let _ = w.set_position(PhysicalPosition::new(x, y));
        std::thread::sleep(Duration::from_millis(15));
    }
    // 最后几个 Moved 事件晚一点才到
    state.quiet_for(200);
}

/// 窗口当前的中心、所在显示器、贴边位置。
fn target<R: Runtime>(app: &AppHandle<R>, w: &WebviewWindow<R>, shown: bool) -> Option<((i32, i32), (i32, i32))> {
    let pos = w.outer_position().ok()?;
    let size = w.outer_size().ok()?;
    let center = (pos.x + size.width as i32 / 2, pos.y + size.height as i32 / 2);
    let (work, scale) = monitor_at(app, center)?;
    let (x, y, _) = place(work, size.width as i32, (GAP_CSS * scale).round() as i32, center, shown);
    Some((center, (x, y)))
}

/// 窗口移动（Moved 事件）：不是动画引起的就当作用户在拖，等松手后贴边并记下位置。
pub fn on_moved<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<BallState>();
    if state.is_quiet() {
        return;
    }
    let mut at = state.dragged_at.lock().expect("悬浮球锁");
    let first = at.is_none();
    *at = Some(Instant::now());
    drop(at);
    if !first {
        return; // 已经有线程在等
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let state = app.state::<BallState>();
        loop {
            std::thread::sleep(Duration::from_millis(60));
            let quiet = state.dragged_at.lock().expect("悬浮球锁").is_some_and(|t| t.elapsed() >= Duration::from_millis(150));
            if quiet && !cursor::left_button_down() {
                break;
            }
        }
        *state.dragged_at.lock().expect("悬浮球锁") = None;
        let Some(w) = app.get_webview_window(LABEL) else { return };
        // 刚松手时指针还在球上：贴边后保持滑出，移开再缩回
        let Some((center, to)) = target(&app, &w, true) else { return };
        animate(&w, &state, to);
        state.shown.store(true, Ordering::SeqCst);
        if let Err(e) = crate::configfile::update(&app, &json!({ "ball": { "pos": [center.0, center.1] } })) {
            eprintln!("记录悬浮球位置失败：{e}");
        }
    });
}

/// 指针移上来 / 移开：滑出或缩回半藏。拖动中不动。
#[tauri::command]
pub fn ball_peek(app: AppHandle, show: bool) {
    let state = app.state::<BallState>();
    if cursor::left_button_down() || state.dragged_at.lock().expect("悬浮球锁").is_some() {
        return;
    }
    if state.shown.swap(show, Ordering::SeqCst) == show {
        return;
    }
    let Some(w) = app.get_webview_window(LABEL) else { return };
    std::thread::spawn(move || {
        let state = app.state::<BallState>();
        if let Some((_, to)) = target(&app, &w, show) {
            animate(&w, &state, to);
        }
    });
}

/// 单击：开始截图。
#[tauri::command]
pub fn ball_click(app: AppHandle) {
    crate::actions::capture(&app);
}

/// 右键：弹出菜单。
#[tauri::command]
pub fn ball_menu(app: AppHandle, window: WebviewWindow) -> Result<(), String> {
    let hk = app.state::<crate::state::AppState>().config.get().hotkeys;
    let label = |name: &str, keys: &str| if keys.trim().is_empty() { name.to_string() } else { format!("{name}\t{}", keys.trim()) };
    let items = (|| -> tauri::Result<Menu<_>> {
        let capture = MenuItem::with_id(&app, "ball-capture", label("截图", &hk.capture), true, None::<&str>)?;
        let pin = MenuItem::with_id(&app, "ball-pin", label("贴图", &hk.pin_clipboard), true, None::<&str>)?;
        let settings = MenuItem::with_id(&app, "ball-settings", "设置…", true, None::<&str>)?;
        let hide = MenuItem::with_id(&app, "ball-hide", "隐藏悬浮球", true, None::<&str>)?;
        Menu::with_items(&app, &[&capture, &pin, &PredefinedMenuItem::separator(&app)?, &settings, &hide])
    })()
    .map_err(|e| e.to_string())?;
    window.popup_menu(&items).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const W: Work = Work { x: 0, y: 0, w: 1920, h: 1040 };

    #[test]
    fn snaps_to_nearer_side_half_hidden() {
        assert_eq!(place(W, 56, 6, (1800, 500), false), (1920 - 28, 472, Side::Right));
        assert_eq!(place(W, 56, 6, (1800, 500), true), (1920 - 56 - 6, 472, Side::Right));
        assert_eq!(place(W, 56, 6, (300, 500), false), (-28, 472, Side::Left));
        assert_eq!(place(W, 56, 6, (300, 500), true), (6, 472, Side::Left));
    }

    #[test]
    fn clamps_vertically_and_works_on_offset_monitors() {
        assert_eq!(place(W, 56, 6, (1800, 5), false).1, 6);
        assert_eq!(place(W, 56, 6, (1800, 5000), false).1, 1040 - 56 - 6);
        let right = Work { x: 1920, y: -200, w: 2560, h: 1400 };
        assert_eq!(place(right, 56, 6, (4400, 0), true), (1920 + 2560 - 62, 0 - 28, Side::Right));
    }
}
