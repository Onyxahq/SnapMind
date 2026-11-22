//! SnapMind 应用外壳：窗口管理、命令、托盘、事件。

mod actions;
mod commands;
mod export;
mod launch;
mod live;
mod long;
mod overlay;
mod pin;
mod pintools;
mod record;
mod providers;
mod state;
mod ocr;
mod selcard;
mod xlsx;
mod translate;
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
                    let is = |s: &str| s.parse::<Shortcut>().is_ok_and(|t| t.id() == shortcut.id());
                    if is(pin::THROUGH_SHORTCUT) {
                        pin::toggle_through(app);
                    } else if is(actions::PIN_CLIPBOARD_SHORTCUT) {
                        actions::pin_clipboard(app);
                    } else if is(actions::TRANSLATE_SHORTCUT) {
                        actions::capture_translate(app);
                    } else if is(actions::RECOGNIZE_SHORTCUT) {
                        actions::capture_recognize(app);
                    } else if is(actions::SELECTION_SHORTCUT) {
                        actions::translate_selection(app);
                    } else {
                        actions::capture(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .manage(overlay::CaptureState::default())
        .manage(pin::PinState::default())
        .manage(pintools::ToolsState::default())
        .manage(translate::TaskState::default())
        .manage(ocr::OcrState::default())
        .manage(selcard::SelState::default())
        .manage(long::LongState::default())
        .manage(live::LiveState::default())
        .manage(record::RecordState::default())
        .register_uri_scheme_protocol(overlay::PROTOCOL, |ctx, req| overlay::serve(ctx.app_handle(), &req))
        .setup(|app| {
            let handle = app.handle();
            let (config, outcome) = ConfigStore::open(&state::config_dir(handle)?)?;
            if let LoadOutcome::Recovered { backup } = &outcome {
                eprintln!("配置文件损坏，已备份到 {} 并恢复默认", backup.display());
            }
            state::apply_native_theme(handle, &config.get());
            let history = match snapmind_core::history::History::open(&state::history_dir(handle)?) {
                Ok(h) => Some(h),
                Err(e) => {
                    eprintln!("打开历史记录失败：{e}");
                    None
                }
            };
            app.manage(state::AppState { config, history });

            tray::create(handle)?;
            overlay::prewarm(handle);
            pintools::prewarm(handle);
            selcard::prewarm(handle);
            std::thread::spawn(export::cleanup_clip_files);
            for key in [
                actions::CAPTURE_SHORTCUT,
                actions::PIN_CLIPBOARD_SHORTCUT,
                actions::TRANSLATE_SHORTCUT,
                actions::RECOGNIZE_SHORTCUT,
                actions::SELECTION_SHORTCUT,
            ] {
                if let Err(e) = handle.global_shortcut().register(key) {
                    // 被别的程序占用时不影响启动；M10.1 在设置页标红提示。
                    eprintln!("注册快捷键 {key} 失败：{e}");
                }
            }
            if launch::mode(std::env::args()) == launch::LaunchMode::Interactive {
                wm::show_settings(handle);
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 划词卡片：点了别处（失去焦点）就收起，固定的除外
            if let WindowEvent::Focused(false) = event {
                if selcard::is_card(window.label()) {
                    selcard::on_blur(window.app_handle(), window.label());
                }
            }
            // 贴图移动或缩放：工具条跟着走
            if let WindowEvent::Moved(_) | WindowEvent::Resized(_) = event {
                if let Some(id) = pin::id_of(window.label()) {
                    pintools::follow(window.app_handle(), id);
                }
            }
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
                } else if selcard::is_card(window.label()) {
                    // Alt+F4 关划词卡片：和点关闭一样（空闲的那张留着下次用）
                    api.prevent_close();
                    selcard::close(window.app_handle(), window.label());
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
            export::capture_pin_text,
            translate::capture_translate,
            translate::capture_translate_overlay,
            translate::capture_markdown,
            translate::capture_table,
            translate::capture_formula,
            translate::translate_text,
            translate::task_cancel,
            ocr::capture_ocr,
            ocr::ocr_compose,
            ocr::ocr_forget,
            ocr::ocr_status,
            ocr::ocr_download,
            ocr::pin_ocr,
            ocr::pin_copy_text,
            selcard::selcard_resize,
            selcard::selcard_pin,
            selcard::selcard_close,
            selcard::selcard_pending,
            commands::pin_info,
            commands::pin_ready,
            commands::pin_zoom,
            commands::pin_close,
            commands::pin_through,
            commands::pin_hover,
            commands::pin_opacity,
            commands::pin_copy,
            commands::pin_save,
            commands::pin_set_image,
            commands::pin_keep,
            commands::pin_set_annotations,
            commands::history_pins,
            commands::history_translations,
            commands::history_delete_translation,
            commands::tools_hover,
            commands::tools_menu,
            commands::tools_layout,
            export::capture_set_annotations,
            long::long_start,
            live::live_hit,
            record::record_start,
            record::record_pause,
            record::record_stop,
            long::long_auto,
            long::long_wheel,
            long::long_preview,
            long::long_cancel,
            long::long_finish,
            export::capture_save,
            export::capture_estimate,
            export::save_info,
            export::pick_save_file,
            export::pick_folder,
            export::save_markdown,
            xlsx::save_xlsx,
            export::open_folder,
            commands::ui_log,
            commands::prompt_defaults,
            commands::languages,
            providers::provider_catalog,
            providers::provider_key_status,
            providers::provider_set_key,
            providers::provider_fetch_models,
            providers::mt_test,
        ])
        .run(tauri::generate_context!())
        .expect("SnapMind 启动失败");
}
