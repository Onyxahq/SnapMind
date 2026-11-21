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

/// 遮罩窗口回报进度（stage = "mask" 遮罩已出现 / "frame" 冻结帧已画好；at = 画出时的
/// Unix 毫秒），返回距按键的毫秒数（已过期的序号返回 None）。
#[tauri::command]
pub fn overlay_ready(app: AppHandle, seq: u64, stage: String, at: f64) -> Option<f64> {
    crate::overlay::ready(&app, seq, &stage, at)
}

/// 吸附链（悬停高亮用）：物理坐标处从最内层控件到顶层窗口的矩形，桌面空白处为空。
#[tauri::command]
pub async fn snap_chain(app: AppHandle, x: i32, y: i32) -> Vec<crate::overlay::SnapRect> {
    tauri::async_runtime::spawn_blocking(move || crate::overlay::snap_chain(&app, x, y))
        .await
        .unwrap_or_default()
}

/// 结束截图：隐藏全部遮罩并释放截图。
#[tauri::command]
pub fn overlay_end(app: AppHandle) {
    crate::overlay::end(&app);
}

/// 前端日志（隐藏窗口里的错误也能看到）。M11.2 起改写到滚动日志文件。
#[tauri::command]
pub fn ui_log(level: String, msg: String) {
    eprintln!("[ui:{level}] {msg}");
}
