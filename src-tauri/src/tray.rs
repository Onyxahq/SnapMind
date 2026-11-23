//! 系统托盘：左键截图，右键菜单。

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};
use snapmind_core::config::HotkeysConfig;
use snapmind_core::i18n::{tr, trf};
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
    pub const UPDATE: &str = "update";
}

/// 托盘里需要随状态变化的菜单项。
pub struct TrayItems<R: Runtime> {
    /// 「取消贴图的鼠标穿透」：有贴图处于穿透状态时才可用。全局快捷键被别的程序占用时，
    /// 这是关掉穿透的唯一入口（穿透的窗口收不到鼠标和键盘）。
    pub unthrough: MenuItem<R>,
    /// 「截图」「贴图」：文字里带着当前的快捷键
    pub capture: MenuItem<R>,
    pub pin: MenuItem<R>,
    pub settings: MenuItem<R>,
    pub quit: MenuItem<R>,
    /// 「开机自启」：设置页改了也要同步勾选状态
    pub autostart: CheckMenuItem<R>,
    /// 整个菜单：有新版本时在最上面插入「更新到 x.y.z…」
    pub menu: Menu<R>,
    /// 有新版本时插进菜单的那一项，连同版本号（换语言时要重写文字）
    pub update: std::sync::Mutex<Option<(MenuItem<R>, String)>>,
}

/// 菜单里显示的快捷键（没设置时不显示）。
fn with_key(label: &str, keys: &str) -> String {
    if keys.trim().is_empty() { label.to_string() } else { format!("{label}\t{}", keys.trim()) }
}

/// 快捷键改了：更新托盘菜单里的提示。
pub fn sync_hotkeys<R: Runtime>(app: &AppHandle<R>, capture: &str, pin: &str) {
    if let Some(items) = app.try_state::<TrayItems<R>>() {
        let _ = items.capture.set_text(with_key(&tr("截图"), capture));
        let _ = items.pin.set_text(with_key(&tr("贴图"), pin));
    }
}

/// 界面语言变了：重写托盘菜单和提示里的文字。
pub fn relabel<R: Runtime>(app: &AppHandle<R>, hk: &HotkeysConfig) {
    let Some(items) = app.try_state::<TrayItems<R>>() else { return };
    let _ = items.capture.set_text(with_key(&tr("截图"), &hk.capture));
    let _ = items.pin.set_text(with_key(&tr("贴图"), &hk.pin_clipboard));
    let _ = items.unthrough.set_text(tr("取消贴图的鼠标穿透"));
    let _ = items.settings.set_text(tr("设置…"));
    let _ = items.autostart.set_text(tr("开机自启"));
    let _ = items.quit.set_text(tr("退出"));
    let version = items.update.lock().expect("托盘锁").as_ref().map(|(item, v)| {
        let _ = item.set_text(update_label(v));
        v.clone()
    });
    set_tooltip(app, version.as_deref());
}

fn update_label(version: &str) -> String {
    trf("更新到 {version}…", &[("version", version)])
}

fn set_tooltip<R: Runtime>(app: &AppHandle<R>, version: Option<&str>) {
    if let Some(tray) = app.tray_by_id("main") {
        let tip = version.map_or("SnapMind".to_string(), |v| trf("SnapMind · 有新版本 {version}", &[("version", v)]));
        let _ = tray.set_tooltip(Some(tip));
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

/// 有新版本：托盘菜单最上面显示「更新到 x.y.z…」（点了打开「关于」页），提示文字也带上。
pub fn set_update<R: Runtime>(app: &AppHandle<R>, version: Option<&str>) {
    let Some(items) = app.try_state::<TrayItems<R>>() else { return };
    let mut cur = items.update.lock().expect("托盘锁");
    match (version, cur.as_mut()) {
        (Some(v), Some((item, shown))) => {
            let _ = item.set_text(update_label(v));
            *shown = v.to_string();
        }
        (Some(v), None) => {
            if let Ok(item) = MenuItem::with_id(app, id::UPDATE, update_label(v), true, None::<&str>) {
                if items.menu.insert(&item, 0).is_ok() {
                    *cur = Some((item, v.to_string()));
                }
            }
        }
        (None, Some((item, _))) => {
            let _ = items.menu.remove(item);
            *cur = None;
        }
        (None, None) => {}
    }
    drop(cur);
    set_tooltip(app, version);
}

/// 有没有贴图处于鼠标穿透状态，更新托盘菜单。
pub fn sync_through<R: Runtime>(app: &AppHandle<R>, any: bool) {
    if let Some(items) = app.try_state::<TrayItems<R>>() {
        let _ = items.unthrough.set_enabled(any);
    }
}

pub fn create<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let autostart_on = app.autolaunch().is_enabled().unwrap_or(false);

    let capture = MenuItem::with_id(app, id::CAPTURE, tr("截图"), true, None::<&str>)?;
    let pin = MenuItem::with_id(app, id::PIN, tr("贴图"), true, None::<&str>)?;
    let unthrough = MenuItem::with_id(app, id::UNTHROUGH, tr("取消贴图的鼠标穿透"), false, None::<&str>)?;
    let settings = MenuItem::with_id(app, id::SETTINGS, tr("设置…"), true, None::<&str>)?;
    let autostart =
        CheckMenuItem::with_id(app, id::AUTOSTART, tr("开机自启"), true, autostart_on, None::<&str>)?;
    let quit = MenuItem::with_id(app, id::QUIT, tr("退出"), true, None::<&str>)?;
    let sep = || PredefinedMenuItem::separator(app);

    let menu = Menu::with_items(
        app,
        &[&capture, &pin, &unthrough, &sep()?, &settings, &autostart, &sep()?, &quit],
    )?;
    app.manage(TrayItems {
        unthrough,
        capture,
        pin,
        settings,
        quit,
        autostart: autostart.clone(),
        menu: menu.clone(),
        update: std::sync::Mutex::new(None),
    });
    let mut builder = TrayIconBuilder::with_id("main")
        .tooltip("SnapMind")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            id::CAPTURE => actions::capture(app),
            id::PIN => actions::pin_clipboard(app),
            id::SETTINGS => wm::show_settings(app, None),
            id::UPDATE => wm::show_settings(app, Some("about")),
            id::UNTHROUGH => crate::pin::clear_through(app),
            id::AUTOSTART => {
                let on = autostart_enabled(app);
                set_autostart(app, !on);
            }
            id::QUIT => {
                crate::pin::close_all_for_exit(app);
                app.exit(0)
            }
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


