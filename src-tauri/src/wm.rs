//! 窗口管理。
//!
//! 模块不叫 `windows`，避免和以后引入的 `windows` crate 重名。
//!
//! 设置窗口按需创建、关掉即销毁（M11.1）：它很少打开，常驻一个隐藏的 WebView 要多占三四十 MB 内存；
//! 打开时多等几百毫秒可以接受（截图遮罩、划词卡片这些要求秒开的仍然预热）。

use snapmind_core::i18n::{tr};
use snapmind_core::config::ThemeMode;
use tauri::window::Color;
use tauri::{AppHandle, Emitter, Manager, Runtime, Theme, WebviewUrl, WebviewWindowBuilder};

use crate::state::AppState;

pub const SETTINGS: &str = "settings";
/// 设置页切到某一页的事件（窗口已经开着时用；新建时用初始化脚本告诉页面）。
const GOTO: &str = "settings:goto";

/// 显示并聚焦设置窗口（最小化时先还原），`page` 指定要打开哪一页。没开着就新建。
/// 可以在任何线程调用：建窗口放到后台线程，避免在主线程的同步命令里卡死。
pub fn show_settings<R: Runtime>(app: &AppHandle<R>, page: Option<&str>) {
    if let Some(w) = app.get_webview_window(SETTINGS) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        if let Some(p) = page {
            let _ = app.emit_to(SETTINGS, GOTO, p);
        }
        return;
    }
    let app = app.clone();
    let page = page.map(str::to_string);
    std::thread::spawn(move || {
        if let Err(e) = build_settings(&app, page.as_deref()) {
            crate::log_warn!("打开设置窗口失败：{e}");
        }
    });
}

fn build_settings<R: Runtime>(app: &AppHandle<R>, page: Option<&str>) -> tauri::Result<()> {
    if app.get_webview_window(SETTINGS).is_some() {
        return Ok(());
    }
    let config = app.state::<AppState>().config.get();
    // 页面画出来之前先用主题底色，深色模式下不会白闪一下
    let dark = match config.appearance.theme {
        ThemeMode::Dark => true,
        ThemeMode::Light => false,
        ThemeMode::System => app.get_webview_window("overlay-0").and_then(|w| w.theme().ok()) == Some(Theme::Dark),
    };
    let bg = if dark { Color(17, 20, 27, 255) } else { Color(238, 241, 246, 255) };
    // 页面名只能是这几个字母，拼进脚本不会出问题
    let page = page.filter(|p| p.chars().all(|c| c.is_ascii_alphabetic())).unwrap_or("");
    let w = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings/index.html".into()))
        .title(tr("SnapMind 设置"))
        .inner_size(1040.0, 720.0)
        .min_inner_size(860.0, 560.0)
        .center()
        .background_color(bg)
        .initialization_script(format!("window.__SETTINGS_PAGE__ = '{page}';"))
        .additional_browser_args(crate::overlay::BROWSER_ARGS)
        .build()?;
    crate::state::apply_native_theme(app, &config);
    let _ = w.set_focus();
    Ok(())
}
