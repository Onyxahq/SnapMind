//! 系统托盘：左键截图，右键菜单。

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;

use crate::{actions, wm};

/// 托盘菜单项 id。
mod id {
    pub const CAPTURE: &str = "capture";
    pub const PIN: &str = "pin";
    pub const SETTINGS: &str = "settings";
    pub const AUTOSTART: &str = "autostart";
    pub const QUIT: &str = "quit";
    pub const UNTHROUGH: &str = "unthrough";
}

/// 托盘里需要随状态变化的菜单项。
pub struct TrayItems<R: Runtime> {
    /// 「取消贴图的鼠标穿透」：有贴图处于穿透状态时才可用。全局快捷键被别的程序占用时，
    /// 这是关掉穿透的唯一入口（穿透的窗口收不到鼠标和键盘）。
    pub unthrough: MenuItem<R>,
    /// 「截图」「贴图」：文字里带着当前的快捷键
    pub capture: MenuItem<R>,
    pub pin: MenuItem<R>,
    /// 「开机自启」：设置页改了也要同步勾选状态
    pub autostart: CheckMenuItem<R>,
}

/// 菜单里显示的快捷键（没设置时不显示）。
fn with_key(label: &str, keys: &str) -> String {
    if keys.trim().is_empty() { label.to_string() } else { format!("{label}\t{}", keys.trim()) }
}

/// 快捷键改了：更新托盘菜单里的提示。
pub fn sync_hotkeys<R: Runtime>(app: &AppHandle<R>, capture: &str, pin: &str) {
    if let Some(items) = app.try_state::<TrayItems<R>>() {
        let _ = items.capture.set_text(with_key("截图", capture));
        let _ = items.pin.set_text(with_key("贴图", pin));
    }
}

/// 开机自启的实际状态（以系统为准）。
pub fn autostart_enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// 设置开机自启，并同步托盘勾选。返回设置后的实际状态。
pub fn set_autostart<R: Runtime>(app: &AppHandle<R>, on: bool) -> bool {
    let al = app.autolaunch();
    let _ = if on { al.enable() } else { al.disable() };
    let now = al.is_enabled().unwrap_or(!on);
    if let Some(items) = app.try_state::<TrayItems<R>>() {
        let _ = items.autostart.set_checked(now);
    }
    now
}

/// 有没有贴图处于鼠标穿透状态，更新托盘菜单。
pub fn sync_through<R: Runtime>(app: &AppHandle<R>, any: bool) {
    if let Some(items) = app.try_state::<TrayItems<R>>() {
        let _ = items.unthrough.set_enabled(any);
    }
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);

    let capture = MenuItem::with_id(app, id::CAPTURE, "截图", true, None::<&str>)?;
    let pin = MenuItem::with_id(app, id::PIN, "贴图", true, None::<&str>)?;
    let unthrough = MenuItem::with_id(app, id::UNTHROUGH, "取消贴图的鼠标穿透", false, None::<&str>)?;
    let settings = MenuItem::with_id(app, id::SETTINGS, "设置…", true, None::<&str>)?;
    let autostart =
        CheckMenuItem::with_id(app, id::AUTOSTART, "开机自启", true, autostart_on, None::<&str>)?;
    let quit = MenuItem::with_id(app, id::QUIT, "退出", true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);

    let menu = Menu::with_items(
        app,
        &[&capture, &pin, &unthrough, &sep()?, &settings, &autostart, &sep()?, &quit],
    )?;
    app.manage(TrayItems { unthrough, capture, pin, autostart: autostart.clone() });
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("SnapMind")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            id::CAPTURE => actions::capture(app),
            id::PIN => actions::pin_clipboard(app),
            id::SETTINGS => wm::show_settings(app),
            id::UNTHROUGH => crate::pin::clear_through(app),
            id::AUTOSTART => {
                let on = autostart_enabled(app);
                set_autostart(app, !on);
            }
            id::QUIT => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                actions::capture(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}


