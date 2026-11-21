//! 全局动作。托盘菜单和全局快捷键（M1 起）都走这里。

use tauri::{AppHandle, Runtime};

/// 截图并进入编辑态。M1.1 实现。
pub fn capture<R: Runtime>(_app: &AppHandle<R>) {}

/// 贴剪贴板内容。M3.2 实现。
pub fn pin_clipboard<R: Runtime>(_app: &AppHandle<R>) {}
