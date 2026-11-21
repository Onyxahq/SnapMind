//! 暴露给前端的命令。

use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AppInfo {
    pub name: &'static str,
    pub version: &'static str,
    pub platform: &'static str,
}

/// 应用基本信息，设置窗口的「关于」页使用。
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: snapmind_core::APP_NAME,
        version: snapmind_core::version(),
        platform: snapmind_platform::platform_name(),
    }
}
