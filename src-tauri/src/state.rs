//! 全局状态与配置变更广播。

use snapmind_core::config::{Config, ConfigStore, ThemeMode};
use tauri::{AppHandle, Emitter, Manager, Runtime, Theme};

/// 配置变更事件名，所有窗口订阅。负载是完整的新配置。
pub const CONFIG_CHANGED: &str = "config-changed";

pub struct AppState {
    pub config: ConfigStore,
}

/// 配置目录：Windows 为 `%APPDATA%\SnapMind`，Linux 为 `~/.config/SnapMind`。
pub fn config_dir<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<std::path::PathBuf> {
    Ok(app.path().config_dir()?.join(snapmind_core::APP_NAME))
}

/// 配置变化后：广播给所有窗口，并同步原生标题栏的明暗。
pub fn on_config_changed<R: Runtime>(app: &AppHandle<R>, config: &Config) {
    let _ = app.emit(CONFIG_CHANGED, config);
    apply_native_theme(app, config);
}

/// 让窗口标题栏、滚动条等系统绘制的部分也跟随设置的明暗。
pub fn apply_native_theme<R: Runtime>(app: &AppHandle<R>, config: &Config) {
    let theme = match config.appearance.theme {
        ThemeMode::System => None,
        ThemeMode::Light => Some(Theme::Light),
        ThemeMode::Dark => Some(Theme::Dark),
    };
    for w in app.webview_windows().values() {
        let _ = w.set_theme(theme);
    }
}
