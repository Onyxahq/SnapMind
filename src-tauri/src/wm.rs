//! 窗口管理。
//!
//! 模块不叫 `windows`，避免和以后引入的 `windows` crate 重名。

use tauri::{AppHandle, Manager, Runtime};

pub const SETTINGS: &str = "settings";

/// 显示并聚焦设置窗口（最小化时先还原）。
pub fn show_settings<R: Runtime>(app: &AppHandle<R>) {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// 哪些窗口点关闭时只隐藏。设置窗口是预先创建的，隐藏后再次打开最快。
pub fn hide_instead_of_close(label: &str) -> bool {
    label == SETTINGS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_window_hides_on_close() {
        assert!(hide_instead_of_close(SETTINGS));
        assert!(!hide_instead_of_close("pin-1"));
    }
}
