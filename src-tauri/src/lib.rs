//! SnapMind 应用外壳：窗口管理、命令、托盘、事件。

mod actions;
mod commands;
mod export;
mod launch;
mod overlay;
mod pin;
mod state;
mod tray;
mod wm;

use snapmind_core::config::{ConfigStore, LoadOutcome};
use tauri::{Manager, WindowEvent};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

pub fn run() {
    tauri::Builder::default()
        // 单实例必须最先注册：重复启动时把参数转给已有实例，再聚焦设置窗口。
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            wm::show_settings(app);
        }))
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args([launch::AUTOSTART_FLAG])
                .build(),
        )
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state != ShortcutState::Pressed {
                        return;
                    }
                    let through: Option<Shortcut> = pin::THROUGH_SHORTCUT.parse().ok();
                    if through.is_some_and(|t| t.id() == shortcut.id()) {
                        pin::toggle_through(app);
                    } else {
                        actions::capture(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .manage(overlay::CaptureState::default())
        .manage(pin::PinState::default())
        .register_uri_scheme_protocol(overlay::PROTOCOL, |ctx, req| overlay::serve(ctx.app_handle(), &req))
        .setup(|app| {
            let handle = app.handle();
            let (config, outcome) = ConfigStore::open(&state::config_dir(handle)?)?;
            if let LoadOutcome::Recovered { backup } = &outcome {
                eprintln!("配置文件损坏，已备份到 {} 并恢复默认", backup.display());
            }
            state::apply_native_theme(handle, &config.get());
            app.manage(state::AppState { config });

            tray::create(handle)?;
            overlay::prewarm(handle);
            std::thread::spawn(export::cleanup_clip_files);
            if let Err(e) = handle.global_shortcut().register(actions::CAPTURE_SHORTCUT) {
                // 被别的程序占用时不影响启动；M10.1 在设置页标红提示。
                eprintln!("注册截图快捷键 {} 失败：{e}", actions::CAPTURE_SHORTCUT);
            }
            if launch::mode(std::env::args()) == launch::LaunchMode::Interactive {
                wm::show_settings(handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 常驻托盘：关闭窗口只是隐藏，退出走托盘菜单。
            if let WindowEvent::CloseRequested { api, .. } = event {
                if let Some(id) = pin::id_of(window.label()) {
                    // 关闭贴图窗口（Alt+F4 等）：释放图片
                    api.prevent_close();
                    pin::close(window.app_handle(), id);
                } else if overlay::is_overlay(window.label()) {
                    // Alt+F4 等关闭遮罩 = 结束截图；窗口保留以便下次秒开。
                    api.prevent_close();
                    overlay::end(window.app_handle());
                } else if wm::hide_instead_of_close(window.label()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::config_get,
            commands::config_update,
            commands::overlay_ready,
            commands::overlay_end,
            commands::snap_chain,
            commands::cursor_nudge,
            commands::clipboard_write_text,
            export::capture_copy,
            export::capture_pin,
            commands::pin_info,
            commands::pin_ready,
            commands::pin_zoom,
            commands::pin_close,
            commands::pin_through,
            export::capture_set_annotations,
            export::capture_save,
            export::capture_estimate,
            export::save_info,
            export::pick_save_file,
            export::pick_folder,
            export::open_folder,
            commands::ui_log,
        ])
        .run(tauri::generate_context!())
        .expect("SnapMind 启动失败");
}
