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
}

/// 有没有贴图处于鼠标穿透状态，更新托盘菜单。
pub fn sync_through<R: Runtime>(app: &AppHandle<R>, any: bool) {
    if let Some(items) = app.try_state::<TrayItems<R>>() {
        let _ = items.unthrough.set_enabled(any);
    }
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);

    let capture = MenuItem::with_id(app, id::CAPTURE, "截图\tShift+F1", true, None::<&str>)?;
    let pin = MenuItem::with_id(app, id::PIN, "贴图\tShift+F3", true, None::<&str>)?;
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
    app.manage(TrayItems { unthrough });

    let autostart_item = autostart.clone();
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("SnapMind")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            id::CAPTURE => actions::capture(app),
            id::PIN => actions::pin_clipboard(app),
            id::SETTINGS => wm::show_settings(app),
            id::UNTHROUGH => crate::pin::clear_through(app),
            id::AUTOSTART => toggle_autostart(app, &autostart_item),
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

/// 切换开机自启。菜单的勾选状态始终以系统里的实际状态为准。
fn toggle_autostart<R: Runtime>(app: &AppHandle<R>, item: &CheckMenuItem<R>) {
    let al = app.autolaunch();
    let was_on = al.is_enabled().unwrap_or(false);
    let _ = if was_on { al.disable() } else { al.enable() };
    let _ = item.set_checked(al.is_enabled().unwrap_or(was_on));
}
