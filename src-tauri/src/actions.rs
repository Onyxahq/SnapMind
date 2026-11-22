//! 全局动作。托盘菜单和全局快捷键都走这里。

use serde::Serialize;
use tauri::{AppHandle, Emitter, Runtime};

/// 默认快捷键（需求第 2 节）。M10.1 起可在设置里修改。
pub const CAPTURE_SHORTCUT: &str = "Shift+F1";
pub const PIN_CLIPBOARD_SHORTCUT: &str = "Shift+F3";
pub const TRANSLATE_SHORTCUT: &str = "Ctrl+Shift+O";
pub const RECOGNIZE_SHORTCUT: &str = "Ctrl+Shift+S";
pub const SELECTION_SHORTCUT: &str = "Ctrl+Alt+D";

/// 截图并进入编辑态。
pub fn capture<R: Runtime>(app: &AppHandle<R>) {
    crate::overlay::begin(app, crate::overlay::Intent::Capture);
}

/// 截图 → 识别 → 翻译：选好区域后直接打开翻译卡片。
pub fn capture_translate<R: Runtime>(app: &AppHandle<R>) {
    crate::overlay::begin(app, crate::overlay::Intent::Translate);
}

/// 截图 → 识别：选好区域后直接识别文字，复制到剪贴板并打开可编辑的识别卡片。
pub fn capture_recognize<R: Runtime>(app: &AppHandle<R>) {
    crate::overlay::begin(app, crate::overlay::Intent::Recognize);
}

/// 取到的选中文字和当时的鼠标位置（物理像素），划词卡片据此出现在鼠标旁边（M7.2）。
#[derive(Debug, Clone, Serialize)]
pub struct SelectionText {
    pub text: String,
    /// automation（UI Automation）/ copy（模拟复制）
    pub method: &'static str,
    pub cursor: (i32, i32),
}

pub const SELECTION_EVENT: &str = "selection:text";

/// 划词翻译（需求第 5 节）：取选中的文字。取词要等用户松开修饰键、可能还要等剪贴板，放在后台线程。
pub fn translate_selection<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    std::thread::spawn(move || {
        let t0 = std::time::Instant::now();
        let cursor = app.cursor_position().map(|c| (c.x as i32, c.y as i32)).unwrap_or((0, 0));
        let Some((text, method)) = snapmind_platform::selection::read(true) else {
            eprintln!("划词：没有取到选中的文字（{:.0} ms）", t0.elapsed().as_secs_f64() * 1e3);
            return;
        };
        let method = match method {
            snapmind_platform::selection::Method::Automation => "automation",
            snapmind_platform::selection::Method::Copy => "copy",
        };
        eprintln!("划词：{method} 取到 {} 字（{:.0} ms）", text.chars().count(), t0.elapsed().as_secs_f64() * 1e3);
        let _ = app.emit(SELECTION_EVENT, SelectionText { text, method, cursor });
    });
}

/// 贴剪贴板内容：图片贴图片，文字渲染成文字卡片；贴图以鼠标位置为中心出现。
pub fn pin_clipboard<R: Runtime>(app: &AppHandle<R>) {
    use snapmind_platform::clipboard::{self, ClipContent};
    let cursor = app.cursor_position().ok();
    let monitor = cursor
        .and_then(|c| app.monitor_from_point(c.x, c.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let scale = monitor.as_ref().map(|m| m.scale_factor()).unwrap_or(1.0);
    let (cx, cy) = cursor.map(|c| (c.x as i32, c.y as i32)).unwrap_or((200, 200));
    let result = match clipboard::read() {
        ClipContent::Image { width, height, rgba } => {
            let at = (cx - width as i32 / 2, cy - height as i32 / 2);
            let img = snapmind_core::export::RgbaImage { width, height, data: rgba };
            crate::pin::create(app, img, snapmind_core::history::PinSource::Clipboard, at, scale)
        }
        // 文字卡片的尺寸要等页面排版后才知道，先把左上角放在鼠标附近
        ClipContent::Text(text) => crate::pin::create_text(app, text, (cx - 40, cy - 30), scale),
        ClipContent::Empty => return,
    };
    if let Err(e) = result {
        eprintln!("贴剪贴板失败：{e}");
    }
}
