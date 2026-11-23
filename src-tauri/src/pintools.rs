//! 贴图工具条（需求第 6 节）：所有贴图共用一个工具条窗口 `pintools`，悬停在哪张贴图上就摆到它下方。
//!
//! 做成独立窗口而不是放进贴图窗口：贴图窗口不必为工具条预留一条透明区域，
//! 工具条隐藏时贴图下方的桌面照常可以点。
//!
//! 防丢失：工具条窗口顶部带 8px 透明连接区；贴图和工具条任意一个处于悬停、或「更多」菜单打开时
//! 一直显示；都离开后延迟约 0.45 秒才隐藏。工具条窗口不抢焦点，贴图里的快捷键照常可用。

use snapmind_core::i18n::{tr};
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindowBuilder};

use crate::pin;

pub const LABEL: &str = "pintools";
/// 都离开后多久隐藏。
const HIDE_DELAY: Duration = Duration::from_millis(450);
/// 贴图和工具条之间的透明连接区（CSS 像素）。页面里有同一个常量。
pub const BRIDGE_CSS: f64 = 8.0;

#[derive(Default)]
pub struct ToolsState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// 当前跟随的贴图
    pin: Option<u32>,
    pin_hover: bool,
    tools_hover: bool,
    menu: bool,
    /// 贴图在标注模式：工具条一直显示（换成标注工具）
    keep: bool,
    /// 每次状态变化加一；延迟隐藏的线程醒来时对比，过期就不隐藏
    gen: u64,
    /// 工具条内容尺寸（CSS 像素，不含连接区），由页面报告
    size: (f64, f64),
    /// 贴图的不透明度和穿透状态，转给工具条显示
    opacity: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Target {
    id: u32,
    /// below：工具条在贴图下方（连接区在上）；above：放不下时在上方（连接区在下，菜单朝上开）
    placement: &'static str,
    opacity: f64,
}

/// 启动时预先建好（隐藏）。
pub fn prewarm<R: Runtime>(app: &AppHandle<R>) {
    let built = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("pintools/index.html".into()))
        .title(tr("SnapMind 贴图工具条"))
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .visible(false)
        .inner_size(300.0, 60.0)
        .additional_browser_args(crate::overlay::BROWSER_ARGS)
        .build();
    match built {
        Ok(w) => {
            if let Ok(hwnd) = w.hwnd() {
                snapmind_platform::window::disable_transitions(hwnd.0 as isize);
                snapmind_platform::window::make_tool_window(hwnd.0 as isize);
            }
        }
        Err(e) => crate::log_warn!("创建贴图工具条窗口失败：{e}"),
    }
}

/// 鼠标进入 / 离开某张贴图。
pub fn pin_hover<R: Runtime>(app: &AppHandle<R>, id: u32, on: bool, opacity: f64) {
    let state = app.state::<ToolsState>();
    {
        let mut s = state.inner.lock().expect("工具条锁");
        if on {
            if s.pin != Some(id) {
                s.menu = false;
            }
            s.pin = Some(id);
            s.pin_hover = true;
            s.opacity = opacity;
        } else if s.pin == Some(id) {
            s.pin_hover = false;
        }
    }
    update(app);
}

/// 鼠标进入 / 离开工具条。
pub fn tools_hover<R: Runtime>(app: &AppHandle<R>, on: bool) {
    app.state::<ToolsState>().inner.lock().expect("工具条锁").tools_hover = on;
    update(app);
}

/// 贴图进入 / 退出标注模式：期间工具条跟随这张贴图且一直显示。
pub fn keep<R: Runtime>(app: &AppHandle<R>, id: u32, on: bool) {
    {
        let state = app.state::<ToolsState>();
        let mut s = state.inner.lock().expect("工具条锁");
        if on {
            s.pin = Some(id);
            s.keep = true;
        } else if s.pin == Some(id) {
            s.keep = false;
        }
    }
    update(app);
}

/// 「更多」菜单打开 / 关闭；菜单打开时工具条不隐藏。
pub fn menu<R: Runtime>(app: &AppHandle<R>, open: bool) {
    app.state::<ToolsState>().inner.lock().expect("工具条锁").menu = open;
    update(app);
}

/// 页面报告内容尺寸（CSS 像素，含菜单展开时的高度）。
pub fn layout<R: Runtime>(app: &AppHandle<R>, width: f64, height: f64) {
    app.state::<ToolsState>().inner.lock().expect("工具条锁").size = (width, height);
    place(app);
}

/// 贴图的不透明度变了（滚轮），同步给工具条里的滑块。
pub fn opacity_changed<R: Runtime>(app: &AppHandle<R>, id: u32, opacity: f64) {
    let state = app.state::<ToolsState>();
    let mut s = state.inner.lock().expect("工具条锁");
    if s.pin == Some(id) {
        s.opacity = opacity;
    }
}

/// 贴图关闭了：不再跟随它。
pub fn forget<R: Runtime>(app: &AppHandle<R>, id: u32) {
    {
        let state = app.state::<ToolsState>();
        let mut s = state.inner.lock().expect("工具条锁");
        if s.pin != Some(id) {
            return;
        }
        *s = Inner { size: s.size, ..Inner::default() };
    }
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.hide();
    }
}

/// 贴图移动或缩放后，工具条跟着走。
pub fn follow<R: Runtime>(app: &AppHandle<R>, id: u32) {
    let current = app.state::<ToolsState>().inner.lock().expect("工具条锁").pin;
    if current == Some(id) {
        place(app);
    }
}

fn update<R: Runtime>(app: &AppHandle<R>) {
    let (active, gen) = {
        let state = app.state::<ToolsState>();
        let mut s = state.inner.lock().expect("工具条锁");
        s.gen += 1;
        (s.pin.is_some() && (s.pin_hover || s.tools_hover || s.menu || s.keep), s.gen)
    };
    let Some(w) = app.get_webview_window(LABEL) else { return };
    if active {
        place(app);
        let _ = w.show();
        return;
    }
    // 延迟隐藏：期间又有悬停，gen 会变，就不隐藏
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(HIDE_DELAY);
        let state = app.state::<ToolsState>();
        let s = state.inner.lock().expect("工具条锁");
        if s.gen == gen {
            let _ = w.hide();
        }
    });
}

/// 工具条窗口的位置（物理像素）：默认在图片下方右对齐，下方放不下就放上方。
/// `image` 为图片区域 (x, y, w, h)，`bar` 为工具条窗口尺寸（含连接区），`screen` 为显示器 (x, y, w, h)。
pub fn placement(
    image: (i32, i32, u32, u32),
    bar: (u32, u32),
    screen: (i32, i32, u32, u32),
) -> ((i32, i32), bool) {
    let (ix, iy, iw, ih) = image;
    let (bw, bh) = bar;
    let (sx, sy, sw, sh) = screen;
    let x = (ix + iw as i32 - bw as i32).clamp(sx, sx + sw as i32 - bw as i32);
    let below = iy + ih as i32;
    if below + bh as i32 <= sy + sh as i32 {
        ((x, below), true)
    } else {
        ((x, (iy - bh as i32).max(sy)), false)
    }
}

fn place<R: Runtime>(app: &AppHandle<R>) {
    let (id, size, opacity) = {
        let state = app.state::<ToolsState>();
        let s = state.inner.lock().expect("工具条锁");
        let Some(id) = s.pin else { return };
        (id, s.size, s.opacity)
    };
    let (Some(tools), Some(pw)) = (app.get_webview_window(LABEL), app.get_webview_window(&pin::label(id))) else {
        return;
    };
    let (Ok(pos), Ok(psize), Ok(scale)) = (pw.outer_position(), pw.inner_size(), pw.scale_factor()) else {
        return;
    };
    let margin = (pin::MARGIN_CSS * scale).round() as i32;
    let image = (pos.x + margin, pos.y + margin, psize.width.saturating_sub(2 * margin as u32), psize.height.saturating_sub(2 * margin as u32));
    let bar = (
        (size.0.max(1.0) * scale).round() as u32,
        ((size.1.max(1.0) + BRIDGE_CSS) * scale).round() as u32,
    );
    let screen = pw
        .current_monitor()
        .ok()
        .flatten()
        .map(|m| (m.position().x, m.position().y, m.size().width, m.size().height))
        .unwrap_or((i32::MIN / 2, i32::MIN / 2, u32::MAX / 2, u32::MAX / 2));
    let ((x, y), below) = placement(image, bar, screen);
    let _ = tools.set_size(PhysicalSize::new(bar.0, bar.1));
    let _ = tools.set_position(PhysicalPosition::new(x, y));
    let _ = app.emit_to(LABEL, "tools:target", Target { id, placement: if below { "below" } else { "above" }, opacity });
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (i32, i32, u32, u32) = (0, 0, 1920, 1080);

    #[test]
    fn below_and_right_aligned() {
        let ((x, y), below) = placement((100, 100, 400, 300), (200, 46), SCREEN);
        assert!(below);
        assert_eq!((x, y), (300, 400));
    }

    #[test]
    fn above_when_no_room_below() {
        let ((_, y), below) = placement((100, 900, 400, 160), (200, 46), SCREEN);
        assert!(!below);
        assert_eq!(y, 854);
    }

    #[test]
    fn clamped_to_screen_horizontally() {
        let ((x, _), _) = placement((-50, 100, 100, 100), (200, 46), SCREEN);
        assert_eq!(x, 0);
    }
}
