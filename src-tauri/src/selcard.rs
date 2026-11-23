//! 划词卡片（需求第 5 节、M7.2）：取到选中的文字后，在鼠标旁边弹出一张小卡片显示译文。
//!
//! - 卡片是独立的透明窗口 `selcard-<n>`。启动时预先建好一张（隐藏），弹出时只是挪位置、显示，
//!   不用等 WebView 加载。
//! - 没固定的卡片失去焦点（点了别处）就隐藏，回到「空闲」等下次使用；固定（图钉）的卡片一直留着，
//!   之后再划词就在最近钉住的那张里接着翻译（F4：不挪位置、不另开卡片）。多出来的卡片关掉时直接销毁。
//! - 位置：鼠标右下方，放不下时翻到鼠标上方，并夹在所在显示器内；页面排版后报告实际高度再调整。
//!   用户拖过的卡片（以及接着用的固定卡片）调整高度时左上角不动，只在超出屏幕底部时往上挪。
//! - 要翻译的文字先存在 Rust 这边，页面收到 `selcard:show` 或刚加载完时来取（只取一次）：新建的卡片
//!   页面还没加载好时发出的事件会丢，靠加载完再取补上。

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindowBuilder};

use crate::actions::SelectionText;

pub const PREFIX: &str = "selcard-";
/// 卡片宽度和初始高度（CSS 像素），页面里有同样的宽度
const WIDTH_CSS: f64 = 380.0;
const HEIGHT_CSS: f64 = 160.0;
/// 卡片离鼠标的距离（CSS 像素）
const GAP_CSS: f64 = 14.0;

#[derive(Default)]
pub struct SelState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    next: u32,
    /// 空闲的卡片（隐藏着，等下次划词）
    free: Option<String>,
    pinned: HashSet<String>,
    /// 最近钉住的卡片：下次划词在它里面接着翻译
    last_pinned: Option<String>,
    /// 用户挪过（或接着用）的卡片：调整高度时不再按鼠标位置摆
    moved: HashSet<String>,
    /// 每张卡片弹出时的鼠标位置和所在显示器（x, y, 宽, 高，物理像素），调整高度时据此重新摆放
    anchor: HashMap<String, Anchor>,
    /// 等页面来取的文字
    pending: HashMap<String, ShowPayload>,
}

#[derive(Debug, Clone, Copy)]
struct Anchor {
    cursor: (i32, i32),
    monitor: (i32, i32, i32, i32),
    scale: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ShowPayload {
    text: String,
    method: &'static str,
}

pub fn is_card(label: &str) -> bool {
    label.starts_with(PREFIX)
}

fn build<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    let state = app.state::<SelState>();
    let label = {
        let mut inner = state.inner.lock().expect("划词卡片锁");
        inner.next += 1;
        format!("{PREFIX}{}", inner.next)
    };
    let built = WebviewWindowBuilder::new(app, &label, WebviewUrl::App("selcard/index.html".into()))
        .title("SnapMind 划词翻译")
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .inner_size(WIDTH_CSS, HEIGHT_CSS)
        .additional_browser_args(crate::overlay::BROWSER_ARGS)
        .build();
    match built {
        Ok(w) => {
            if let Ok(hwnd) = w.hwnd() {
                snapmind_platform::window::disable_transitions(hwnd.0 as isize);
            }
            Some(label)
        }
        Err(e) => {
            crate::log_warn!("创建划词卡片失败：{e}");
            None
        }
    }
}

/// 启动时预先建一张空闲卡片。
pub fn prewarm<R: Runtime>(app: &AppHandle<R>) {
    if let Some(label) = build(app) {
        app.state::<SelState>().inner.lock().expect("划词卡片锁").free = Some(label);
    }
}

/// 卡片位置：鼠标右下方；下方放不下翻到上方；夹在显示器内。返回左上角（物理像素）。
pub fn place(cursor: (i32, i32), monitor: (i32, i32, i32, i32), size: (i32, i32), gap: i32) -> (i32, i32) {
    let (mx, my, mw, mh) = monitor;
    let (w, h) = size;
    let margin = gap / 2;
    let x = (cursor.0 + gap).min(mx + mw - w - margin).max(mx + margin);
    let below = cursor.1 + gap;
    let y = if below + h <= my + mh - margin { below } else { (cursor.1 - gap - h).max(my + margin) };
    (x, y)
}

/// 挪过的卡片换了高度：左上角不动，超出所在显示器底部时往上挪（但不越过顶部）。
pub fn keep_in(pos: (i32, i32), height: i32, monitor: (i32, i32, i32, i32), margin: i32) -> (i32, i32) {
    let (_, my, _, mh) = monitor;
    let bottom = my + mh - margin;
    let y = if pos.1 + height > bottom { (bottom - height).max(my + margin) } else { pos.1 };
    (pos.0, y)
}

fn apply<R: Runtime>(app: &AppHandle<R>, label: &str, height_css: f64) {
    let (anchor, moved) = {
        let state = app.state::<SelState>();
        let inner = state.inner.lock().expect("划词卡片锁");
        (inner.anchor.get(label).copied(), inner.moved.contains(label))
    };
    let Some(win) = app.get_webview_window(label) else { return };
    if moved {
        let scale = win.scale_factor().unwrap_or(1.0);
        let size = ((WIDTH_CSS * scale).round() as i32, (height_css * scale).round() as i32);
        let Ok(pos) = win.outer_position() else { return };
        let monitor = win
            .current_monitor()
            .ok()
            .flatten()
            .map(|m| (m.position().x, m.position().y, m.size().width as i32, m.size().height as i32));
        let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
        if let Some(mon) = monitor {
            let (x, y) = keep_in((pos.x, pos.y), size.1, mon, (GAP_CSS * scale / 2.0).round() as i32);
            if (x, y) != (pos.x, pos.y) {
                let _ = win.set_position(PhysicalPosition::new(x, y));
            }
        }
        return;
    }
    let Some(a) = anchor else { return };
    let size = ((WIDTH_CSS * a.scale).round() as i32, (height_css * a.scale).round() as i32);
    let (x, y) = place(a.cursor, a.monitor, size, (GAP_CSS * a.scale).round() as i32);
    let _ = win.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    let _ = win.set_position(PhysicalPosition::new(x, y));
}

/// 在鼠标旁弹出卡片显示这段文字（在取词的后台线程里调用）。有钉住的卡片时在它里面接着翻译。
pub fn show<R: Runtime>(app: &AppHandle<R>, sel: SelectionText) {
    let reuse = {
        let state = app.state::<SelState>();
        let mut inner = state.inner.lock().expect("划词卡片锁");
        let hit = inner.last_pinned.clone().filter(|l| inner.pinned.contains(l) && app.get_webview_window(l).is_some());
        if let Some(l) = &hit {
            inner.moved.insert(l.clone());
            inner.pending.insert(l.clone(), ShowPayload { text: sel.text.clone(), method: sel.method });
        }
        hit
    };
    if let Some(label) = reuse {
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.emit_to(label.as_str(), "selcard:show", ());
            let _ = win.unminimize();
            let _ = win.show();
            let _ = win.set_focus();
        }
        return;
    }
    let free = app.state::<SelState>().inner.lock().expect("划词卡片锁").free.take();
    let Some(label) = free.or_else(|| build(app)) else { return };
    let Some(win) = app.get_webview_window(&label) else { return };
    let (cx, cy) = sel.cursor;
    let mon = app
        .monitor_from_point(cx as f64, cy as f64)
        .ok()
        .flatten()
        .or_else(|| app.primary_monitor().ok().flatten());
    let (monitor, scale) = match &mon {
        Some(m) => {
            let (p, s) = (m.position(), m.size());
            ((p.x, p.y, s.width as i32, s.height as i32), m.scale_factor())
        }
        None => ((0, 0, 1920, 1080), 1.0),
    };
    app.state::<SelState>()
        .inner
        .lock()
        .expect("划词卡片锁")
        .anchor
        .insert(label.clone(), Anchor { cursor: (cx, cy), monitor, scale });
    apply(app, &label, HEIGHT_CSS);
    app.state::<SelState>()
        .inner
        .lock()
        .expect("划词卡片锁")
        .pending
        .insert(label.clone(), ShowPayload { text: sel.text, method: sel.method });
    let _ = win.emit_to(label.as_str(), "selcard:show", ());
    let _ = win.show();
    let _ = win.set_focus();
}

/// 页面排版后报告高度。
pub fn resize<R: Runtime>(app: &AppHandle<R>, label: &str, height_css: f64) {
    apply(app, label, height_css.clamp(60.0, 640.0));
}

pub fn set_pinned<R: Runtime>(app: &AppHandle<R>, label: &str, on: bool) {
    let need_free = {
        let state = app.state::<SelState>();
        let mut inner = state.inner.lock().expect("划词卡片锁");
        if on {
            inner.pinned.insert(label.to_string());
            inner.last_pinned = Some(label.to_string());
            if inner.free.as_deref() == Some(label) {
                inner.free = None;
            }
        } else {
            inner.pinned.remove(label);
            if inner.last_pinned.as_deref() == Some(label) {
                inner.last_pinned = inner.pinned.iter().next().cloned();
            }
        }
        inner.free.is_none()
    };
    // 这张被钉住了：马上预备下一张空闲卡片，下次划词不用等页面加载（建窗口不能在主线程的同步命令里做）
    if on && need_free {
        let app = app.clone();
        std::thread::spawn(move || {
            if let Some(label) = build(&app) {
                let state = app.state::<SelState>();
                let mut inner = state.inner.lock().expect("划词卡片锁");
                if inner.free.is_none() {
                    inner.free = Some(label);
                } else {
                    drop(inner);
                    if let Some(w) = app.get_webview_window(&label) {
                        let _ = w.destroy();
                    }
                }
            }
        });
    }
}

/// 页面来取要翻译的文字（取走后就没有了）。
pub fn take_pending<R: Runtime>(app: &AppHandle<R>, label: &str) -> Option<ShowPayload> {
    app.state::<SelState>().inner.lock().expect("划词卡片锁").pending.remove(label)
}

/// 失去焦点：没固定就收起。
pub fn on_blur<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let pinned = app.state::<SelState>().inner.lock().expect("划词卡片锁").pinned.contains(label);
    if !pinned {
        close(app, label);
    }
}

/// 收起卡片：没有空闲卡片时它回到空闲（隐藏），否则销毁。页面收到 selcard:hide 后取消翻译、清空。
pub fn close<R: Runtime>(app: &AppHandle<R>, label: &str) {
    let Some(win) = app.get_webview_window(label) else { return };
    let keep = {
        let state = app.state::<SelState>();
        let mut inner = state.inner.lock().expect("划词卡片锁");
        inner.pinned.remove(label);
        if inner.last_pinned.as_deref() == Some(label) {
            inner.last_pinned = inner.pinned.iter().next().cloned();
        }
        inner.moved.remove(label);
        inner.anchor.remove(label);
        inner.pending.remove(label);
        match inner.free.as_deref() {
            None => {
                inner.free = Some(label.to_string());
                true
            }
            Some(f) => f == label,
        }
    };
    let _ = win.emit_to(label, "selcard:hide", ());
    if keep {
        let _ = win.hide();
    } else {
        let _ = win.destroy();
    }
}

#[tauri::command]
pub fn selcard_resize(app: AppHandle, window: tauri::WebviewWindow, height: f64) {
    resize(&app, window.label(), height);
}

#[tauri::command]
pub fn selcard_pin(app: AppHandle, window: tauri::WebviewWindow, on: bool) {
    set_pinned(&app, window.label(), on);
}

/// 页面上按下鼠标要拖动卡片：记下「用户挪过」，再交给系统拖动（跟手、能拖到别的显示器）。
#[tauri::command]
pub async fn selcard_drag(app: AppHandle, window: tauri::WebviewWindow) {
    app.state::<SelState>().inner.lock().expect("划词卡片锁").moved.insert(window.label().to_string());
    let _ = window.start_dragging();
}

#[tauri::command]
pub fn selcard_pending(app: AppHandle, window: tauri::WebviewWindow) -> Option<ShowPayload> {
    take_pending(&app, window.label())
}

#[tauri::command]
pub fn selcard_close(app: AppHandle, window: tauri::WebviewWindow) {
    close(&app, window.label());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn places_beside_cursor_and_flips_or_clamps() {
        let mon = (0, 0, 1920, 1080);
        // 有地方：右下方
        assert_eq!(place((100, 100), mon, (380, 160), 14), (114, 114));
        // 靠右边：夹在屏幕内
        assert_eq!(place((1900, 100), mon, (380, 160), 14), (1920 - 380 - 7, 114));
        // 靠下边：翻到鼠标上方
        assert_eq!(place((100, 1000), mon, (380, 160), 14), (114, 1000 - 14 - 160));
        // 第二块屏在左边（负坐标）
        assert_eq!(place((-500, 50), (-1920, 0, 1920, 1080), (380, 160), 14), (-486, 64));
    }

    #[test]
    fn moved_card_keeps_its_corner_unless_it_would_fall_off() {
        let mon = (0, 0, 1920, 1080);
        // 放得下：原地变高
        assert_eq!(keep_in((600, 300), 400, mon, 7), (600, 300));
        // 变高后超出底部：往上挪到贴着底边
        assert_eq!(keep_in((600, 900), 400, mon, 7), (600, 1080 - 7 - 400));
        // 比屏幕还高：顶到上边为止
        assert_eq!(keep_in((600, 900), 2000, mon, 7), (600, 7));
    }
}
