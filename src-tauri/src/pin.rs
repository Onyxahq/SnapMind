//! 贴图窗口：把截图钉在屏幕上（需求第 6 节）。
//!
//! - 每张贴图一个窗口 `pin-<id>`：透明、无边框、置顶、不进任务栏；图片四周留一圈透明边距画阴影。
//! - 图片按 1:1 物理像素出现在原来截图的位置；图片数据存在内存里，经 `snapframe` 协议的
//!   `/pin/<id>` 提供给页面（BMP，无需编码）。
//! - 缩放由 Rust 改窗口大小和位置（以鼠标为中心）；当前缩放比例总是从窗口实际尺寸推算，
//!   拖到不同缩放的显示器上被系统改了尺寸也不会错乱。
//! - 鼠标穿透后窗口收不到按键，所以 `Ctrl+Shift+X` 注册为全局快捷键（有贴图时才注册）：
//!   焦点在某张贴图上时让它穿透，否则取消所有贴图的穿透。

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use snapmind_core::export::RgbaImage;
use snapmind_platform::bmp::Frame;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub const LABEL_PREFIX: &str = "pin-";
/// 鼠标穿透的开关键。
pub const THROUGH_SHORTCUT: &str = "Ctrl+Shift+X";
/// 图片四周的透明边距（CSS 像素），用来画阴影。页面里有同一个常量。
pub const MARGIN_CSS: f64 = 10.0;
/// 缩放范围。
pub const ZOOM_MIN: f64 = 0.1;
pub const ZOOM_MAX: f64 = 8.0;

pub struct Pin {
    /// 供页面显示的 BMP
    pub frame: Arc<Frame>,
    /// 原始像素（复制、保存、识别用）
    pub image: Arc<RgbaImage>,
    pub through: bool,
}

#[derive(Default)]
pub struct PinState {
    next: AtomicU32,
    pins: Mutex<HashMap<u32, Pin>>,
}

#[derive(Debug, Serialize)]
pub struct PinInfo {
    pub width: u32,
    pub height: u32,
}

pub fn label(id: u32) -> String {
    format!("{LABEL_PREFIX}{id}")
}

pub fn id_of(label: &str) -> Option<u32> {
    label.strip_prefix(LABEL_PREFIX)?.parse().ok()
}

/// 由当前窗口宽度推算缩放比例：(窗口宽 − 两侧边距) / 图片宽。
pub fn zoom_of(window_width: u32, margin_phys: u32, image_width: u32) -> f64 {
    (window_width.saturating_sub(2 * margin_phys)) as f64 / image_width.max(1) as f64
}

/// 以锚点（窗口内物理像素）为中心缩放后，窗口新的位置和大小。
pub fn zoom_geometry(
    pos: (i32, i32),
    margin: u32,
    image: (u32, u32),
    old_zoom: f64,
    new_zoom: f64,
    anchor: (f64, f64),
) -> ((i32, i32), (u32, u32)) {
    let w = (image.0 as f64 * new_zoom).round().max(1.0) as u32 + 2 * margin;
    let h = (image.1 as f64 * new_zoom).round().max(1.0) as u32 + 2 * margin;
    // 锚点相对图片左上角的位置按比例缩放，保持锚点在屏幕上不动
    let k = new_zoom / old_zoom;
    let ax = anchor.0 - margin as f64;
    let ay = anchor.1 - margin as f64;
    let x = pos.0 as f64 + ax - ax * k;
    let y = pos.1 as f64 + ay - ay * k;
    ((x.round() as i32, y.round() as i32), (w, h))
}

fn rgba_to_bgra(img: &RgbaImage) -> Vec<u8> {
    img.data.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()
}

/// 新建贴图。`at` 是图片左上角的虚拟桌面物理坐标，`scale` 是所在显示器的缩放。
pub fn create<R: Runtime>(app: &AppHandle<R>, image: RgbaImage, at: (i32, i32), scale: f64) -> tauri::Result<u32> {
    let state = app.state::<PinState>();
    let id = state.next.fetch_add(1, Ordering::SeqCst) + 1;
    let (w, h) = (image.width, image.height);
    let frame = Arc::new(Frame::from_bgra(w, h, &rgba_to_bgra(&image)));
    state.pins.lock().expect("贴图锁").insert(id, Pin { frame, image: Arc::new(image), through: false });

    let margin = (MARGIN_CSS * scale).round() as i32;
    let win = WebviewWindowBuilder::new(app, label(id), WebviewUrl::App("pin/index.html".into()))
        .title("SnapMind 贴图")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .additional_browser_args(crate::overlay::BROWSER_ARGS)
        .build()?;
    let _ = win.set_position(PhysicalPosition::new(at.0 - margin, at.1 - margin));
    let _ = win.set_size(PhysicalSize::new(w + 2 * margin as u32, h + 2 * margin as u32));
    if let Ok(hwnd) = win.hwnd() {
        snapmind_platform::window::disable_transitions(hwnd.0 as isize);
    }
    ensure_shortcut(app);
    Ok(id)
}

/// 页面画好后显示窗口并给它焦点（避免透明窗口先闪一下空白）。
pub fn ready<R: Runtime>(app: &AppHandle<R>, id: u32) {
    if let Some(w) = app.get_webview_window(&label(id)) {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn info<R: Runtime>(app: &AppHandle<R>, id: u32) -> Option<PinInfo> {
    let state = app.state::<PinState>();
    let pins = state.pins.lock().expect("贴图锁");
    pins.get(&id).map(|p| PinInfo { width: p.image.width, height: p.image.height })
}

pub fn frame<R: Runtime>(app: &AppHandle<R>, id: u32) -> Option<Arc<Frame>> {
    let state = app.state::<PinState>();
    let pins = state.pins.lock().expect("贴图锁");
    pins.get(&id).map(|p| p.frame.clone())
}

/// 缩放。`factor` 为相对倍数（滚轮）；`absolute` 为目标比例（Ctrl+0 → 1.0）。
/// 锚点是窗口内的 CSS 坐标。返回缩放后的比例。
pub fn zoom<R: Runtime>(
    app: &AppHandle<R>,
    id: u32,
    factor: Option<f64>,
    absolute: Option<f64>,
    anchor_css: (f64, f64),
) -> Option<f64> {
    let image = info(app, id)?;
    let w = app.get_webview_window(&label(id))?;
    let scale = w.scale_factor().ok()?;
    let margin = (MARGIN_CSS * scale).round() as u32;
    let size = w.inner_size().ok()?;
    let pos = w.outer_position().ok()?;
    let old = zoom_of(size.width, margin, image.width);
    let target = absolute.unwrap_or(old * factor.unwrap_or(1.0)).clamp(ZOOM_MIN, ZOOM_MAX);
    if (target - old).abs() < 1e-4 {
        return Some(old);
    }
    let anchor = (anchor_css.0 * scale, anchor_css.1 * scale);
    let (p, s) = zoom_geometry((pos.x, pos.y), margin, (image.width, image.height), old, target, anchor);
    let _ = w.set_position(PhysicalPosition::new(p.0, p.1));
    let _ = w.set_size(PhysicalSize::new(s.0, s.1));
    Some(target)
}

/// 设置一张贴图的鼠标穿透，并同步托盘菜单。
pub fn set_through<R: Runtime>(app: &AppHandle<R>, id: u32, on: bool) {
    if let Some(w) = app.get_webview_window(&label(id)) {
        let _ = w.set_ignore_cursor_events(on);
        let _ = tauri::Emitter::emit_to(app, label(id), "pin:through", on);
    }
    let any = {
        let state = app.state::<PinState>();
        let mut pins = state.pins.lock().expect("贴图锁");
        if let Some(p) = pins.get_mut(&id) {
            p.through = on;
        }
        pins.values().any(|p| p.through)
    };
    crate::tray::sync_through(app, any);
}

/// 取消所有贴图的鼠标穿透（全局快捷键、托盘菜单）。
pub fn clear_through<R: Runtime>(app: &AppHandle<R>) {
    let ids: Vec<u32> = {
        let state = app.state::<PinState>();
        let pins = state.pins.lock().expect("贴图锁");
        pins.iter().filter(|(_, p)| p.through).map(|(id, _)| *id).collect()
    };
    for id in ids {
        set_through(app, id, false);
    }
}

/// 全局 Ctrl+Shift+X：焦点在某张贴图上 → 让它穿透；否则 → 取消所有贴图的穿透。
pub fn toggle_through<R: Runtime>(app: &AppHandle<R>) {
    let focused = app
        .webview_windows()
        .into_iter()
        .filter_map(|(l, w)| id_of(&l).filter(|_| w.is_focused().unwrap_or(false)))
        .next();
    match focused {
        Some(id) => set_through(app, id, true),
        None => clear_through(app),
    }
}

/// 关闭贴图：销毁窗口、释放图片；没有贴图了就注销穿透快捷键。M3.4 起关闭时写入历史。
pub fn close<R: Runtime>(app: &AppHandle<R>, id: u32) {
    let (empty, any_through) = {
        let state = app.state::<PinState>();
        let mut pins = state.pins.lock().expect("贴图锁");
        pins.remove(&id);
        (pins.is_empty(), pins.values().any(|p| p.through))
    };
    crate::tray::sync_through(app, any_through);
    if let Some(w) = app.get_webview_window(&label(id)) {
        let _ = w.destroy();
    }
    if empty {
        let _ = app.global_shortcut().unregister(THROUGH_SHORTCUT);
    }
}

fn ensure_shortcut<R: Runtime>(app: &AppHandle<R>) {
    let gs = app.global_shortcut();
    if !gs.is_registered(THROUGH_SHORTCUT) {
        if let Err(e) = gs.register(THROUGH_SHORTCUT) {
            // 被别的程序占用：贴图窗口自己处理按键开启穿透，托盘菜单负责取消
            eprintln!("注册穿透快捷键 {THROUGH_SHORTCUT} 失败（改由贴图窗口和托盘菜单处理）：{e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(label(3), "pin-3");
        assert_eq!(id_of("pin-12"), Some(12));
        assert_eq!(id_of("overlay-0"), None);
    }

    #[test]
    fn zoom_from_window_size() {
        assert_eq!(zoom_of(520, 10, 500), 1.0);
        assert_eq!(zoom_of(1020, 10, 500), 2.0);
    }

    #[test]
    fn zoom_keeps_anchor_fixed() {
        // 500×300 的图片，边距 10，窗口在 (100,100)；在图片中心 (260,160) 处放大到 2 倍
        let ((x, y), (w, h)) = zoom_geometry((100, 100), 10, (500, 300), 1.0, 2.0, (260.0, 160.0));
        assert_eq!((w, h), (1020, 620));
        // 锚点原来在屏幕 (360,260)，放大后仍在同一位置
        assert_eq!((x + 10 + 500, y + 10 + 300), (360, 260));
    }

    #[test]
    fn zoom_at_top_left_corner_of_image() {
        let ((x, y), _) = zoom_geometry((0, 0), 10, (100, 100), 1.0, 3.0, (10.0, 10.0));
        assert_eq!((x, y), (0, 0), "锚点在图片左上角时窗口不动");
    }
}
