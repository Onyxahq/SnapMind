//! 全局动作。托盘菜单和全局快捷键都走这里。

use tauri::{AppHandle, Runtime};

/// 默认截图快捷键（需求第 2 节）。M10.1 起可在设置里修改。
pub const CAPTURE_SHORTCUT: &str = "Shift+F1";

/// 截图并进入编辑态。
pub fn capture<R: Runtime>(app: &AppHandle<R>) {
    crate::overlay::begin(app);
}

/// 贴剪贴板内容。M3.2 实现。
pub fn pin_clipboard<R: Runtime>(_app: &AppHandle<R>) {}
