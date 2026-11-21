//! SnapMind 应用外壳：窗口管理、命令、托盘、事件。

mod commands;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::app_info])
        .run(tauri::generate_context!())
        .expect("SnapMind 启动失败");
}
