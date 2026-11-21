//! SnapMind 应用外壳：窗口管理、命令、托盘、事件。

mod actions;
mod commands;
mod launch;
mod tray;
mod wm;

use tauri::WindowEvent;

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
        .setup(|app| {
            tray::create(app.handle())?;
            if launch::mode(std::env::args()) == launch::LaunchMode::Interactive {
                wm::show_settings(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 常驻托盘：关闭窗口只是隐藏，退出走托盘菜单。
            if let WindowEvent::CloseRequested { api, .. } = event {
                if wm::hide_instead_of_close(window.label()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("SnapMind 启动失败");
}
