//! 暴露给前端的命令。

use serde::Serialize;
use serde_json::Value;
use snapmind_core::config::Config;
use tauri::{AppHandle, State};

use crate::state::{self, AppState};

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

/// 读取当前完整配置。
#[tauri::command]
pub fn config_get(state: State<'_, AppState>) -> Config {
    state.config.get()
}

/// 用 JSON Merge Patch 修改配置。校验失败时返回错误信息，配置保持不变；
/// 成功且内容有变化时广播 `config-changed`。返回修改后的完整配置。
#[tauri::command]
pub fn config_update(app: AppHandle, state: State<'_, AppState>, patch: Value) -> Result<Config, String> {
    match state.config.update(&patch).map_err(|e| e.to_string())? {
        Some(next) => {
            state::on_config_changed(&app, &next);
            Ok(next)
        }
        None => Ok(state.config.get()),
    }
}
