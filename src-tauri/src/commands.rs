//! 暴露给前端的命令。

use serde::Serialize;
use serde_json::Value;
use snapmind_core::config::Config;
use tauri::{AppHandle, Manager, State};

use crate::state::AppState;

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
pub fn config_update(app: AppHandle, patch: Value) -> Result<Config, String> {
    crate::configfile::update(&app, &patch)
}

/// 配置文件的完整路径（设置 → 配置文件）。
#[tauri::command]
pub fn config_path(state: State<'_, AppState>) -> String {
    state.config.path().to_string_lossy().into_owned()
}

/// 打开设置窗口，可以指定跳到哪一页（设置页收到 settings:goto 事件后切换）。
#[tauri::command]
pub fn settings_open(app: AppHandle, page: Option<String>) {
    crate::wm::show_settings(&app, page.as_deref());
}

/// 开机自启的实际状态。
#[tauri::command]
pub fn autostart_get(app: AppHandle) -> bool {
    crate::tray::autostart_enabled(&app)
}

/// 设置开机自启（托盘勾选同步），返回实际状态。
#[tauri::command]
pub fn autostart_set(app: AppHandle, on: bool) -> bool {
    crate::tray::set_autostart(&app, on)
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

/// 把系统鼠标移动若干物理像素（截图时 W A S D 逐像素移动光标）。
#[tauri::command]
pub fn cursor_nudge(dx: i32, dy: i32) {
    snapmind_platform::cursor::nudge(dx, dy);
}

/// 把文字写入剪贴板（取色值等）。
#[tauri::command]
pub fn clipboard_write_text(text: String) -> Result<(), String> {
    snapmind_platform::clipboard::set_text(&text).map_err(|e| e.to_string())
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

/// 贴图页面启动时取图片尺寸。
#[tauri::command]
pub fn pin_info(app: AppHandle, window: tauri::WebviewWindow) -> Option<crate::pin::PinInfo> {
    crate::pin::id_of(window.label()).and_then(|id| crate::pin::info(&app, id))
}

/// 贴图页面画好后调用，显示窗口。
#[tauri::command]
pub fn pin_ready(app: AppHandle, window: tauri::WebviewWindow) {
    if let Some(id) = crate::pin::id_of(window.label()) {
        crate::pin::ready(&app, id);
    }
}

/// 缩放贴图：factor 为相对倍数（滚轮），absolute 为目标比例（Ctrl+0）；锚点为窗口内 CSS 坐标。
#[tauri::command]
pub fn pin_zoom(
    app: AppHandle,
    window: tauri::WebviewWindow,
    factor: Option<f64>,
    absolute: Option<f64>,
    x: f64,
    y: f64,
) -> Option<f64> {
    let id = crate::pin::id_of(window.label())?;
    crate::pin::zoom(&app, id, factor, absolute, (x, y))
}

/// 关闭贴图（双击、Esc）。
#[tauri::command]
pub fn pin_close(app: AppHandle, window: tauri::WebviewWindow) {
    if let Some(id) = crate::pin::id_of(window.label()) {
        crate::pin::close(&app, id);
    }
}

/// 让当前贴图鼠标穿透（全局快捷键注册失败时由贴图页面的按键触发）。
#[tauri::command]
pub fn pin_through(app: AppHandle, window: tauri::WebviewWindow) {
    if let Some(id) = crate::pin::id_of(window.label()) {
        crate::pin::set_through(&app, id, true);
    }
}

/// 贴图悬停状态（工具条据此显示 / 延迟隐藏），同时带上当前不透明度。
#[tauri::command]
pub fn pin_hover(app: AppHandle, window: tauri::WebviewWindow, on: bool, opacity: f64) {
    if let Some(id) = crate::pin::id_of(window.label()) {
        crate::pintools::pin_hover(&app, id, on, opacity);
    }
}

/// 贴图不透明度变化（同步工具条滑块）。
#[tauri::command]
pub fn pin_opacity(app: AppHandle, window: tauri::WebviewWindow, opacity: f64) {
    if let Some(id) = crate::pin::id_of(window.label()) {
        crate::pintools::opacity_changed(&app, id, opacity);
    }
}

/// 工具条悬停状态。
#[tauri::command]
pub fn tools_hover(app: AppHandle, on: bool) {
    crate::pintools::tools_hover(&app, on);
}

/// 工具条「更多」菜单开关。
#[tauri::command]
pub fn tools_menu(app: AppHandle, open: bool) {
    crate::pintools::menu(&app, open);
}

/// 工具条报告内容尺寸（CSS 像素）。
#[tauri::command]
pub fn tools_layout(app: AppHandle, width: f64, height: f64) {
    crate::pintools::layout(&app, width, height);
}

/// 复制贴图（文字卡片复制文字）。
#[tauri::command]
pub async fn pin_copy(app: AppHandle, state: State<'_, AppState>, id: u32) -> Result<(), String> {
    let template = state.config.get().save.filename_template;
    tauri::async_runtime::spawn_blocking(move || crate::pin::copy(&app, id, &template))
        .await
        .map_err(|e| e.to_string())?
}

/// 保存贴图：系统另存为对话框（挂在贴图窗口下），格式按扩展名，默认目录和文件名模板与截图一致。
#[tauri::command]
pub async fn pin_save(app: AppHandle, state: State<'_, AppState>, id: u32) -> Result<Option<String>, String> {
    use snapmind_core::export::{self, ImageFormat};
    use tauri_plugin_dialog::DialogExt;
    let image = crate::pin::image(&app, id).ok_or("贴图已关闭")?;
    let window = app.get_webview_window(&crate::pin::label(id)).ok_or("贴图已关闭")?;
    let config = state.config.get();
    let dir = crate::export::default_dir(&app, &config);
    let name = export::render_filename(&config.save.filename_template, export::now_local());
    let fmt = config.save.format;
    let quality = config.save.quality;
    tauri::async_runtime::spawn_blocking(move || {
        let picked = window
            .dialog()
            .file()
            .set_parent(&window)
            .set_directory(&dir)
            .set_file_name(format!("{name}.{}", fmt.extension()))
            .add_filter("PNG", &["png"])
            .add_filter("JPG", &["jpg", "jpeg"])
            .add_filter("WebP", &["webp"])
            .blocking_save_file()
            .and_then(|p| p.into_path().ok());
        let Some(path) = picked else { return Ok(None) };
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let format = match ext.as_str() {
            "jpg" | "jpeg" => ImageFormat::Jpg,
            "webp" => ImageFormat::Webp,
            _ => ImageFormat::Png,
        };
        let bytes = export::encode(&image, format, quality, false).map_err(|e| e.to_string())?;
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 文字卡片排版完成：页面把卡片渲染成 RGBA 交回（二进制请求体，请求头 x-width / x-height）。
#[tauri::command]
pub fn pin_set_image(app: AppHandle, window: tauri::WebviewWindow, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let id = crate::pin::id_of(window.label()).ok_or("不是贴图窗口")?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err("需要二进制数据".into()) };
    let header = |n: &str| request.headers().get(n).and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u32>().ok());
    let (w, h) = (header("x-width").unwrap_or(0), header("x-height").unwrap_or(0));
    if w == 0 || h == 0 || (w * h * 4) as usize != bytes.len() {
        return Err("卡片图片尺寸不对".into());
    }
    crate::pin::set_image(&app, id, snapmind_core::export::RgbaImage { width: w, height: h, data: bytes.clone() });
    Ok(())
}

/// 贴图进入 / 退出标注模式（工具条换成标注工具并保持显示）。
#[tauri::command]
pub fn pin_keep(app: AppHandle, window: tauri::WebviewWindow, on: bool) {
    if let Some(id) = crate::pin::id_of(window.label()) {
        crate::pintools::keep(&app, id, on);
    }
}

/// 上传贴图的标注层：RGBA（为空表示清除），请求头 x-x / x-y / x-width / x-height。
#[tauri::command]
pub fn pin_set_annotations(app: AppHandle, window: tauri::WebviewWindow, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let id = crate::pin::id_of(window.label()).ok_or("不是贴图窗口")?;
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else { return Err("需要二进制数据".into()) };
    if bytes.is_empty() {
        crate::pin::set_annotations(&app, id, None);
        return Ok(());
    }
    let header = |n: &str| request.headers().get(n).and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u32>().ok());
    let (w, h) = (header("x-width").unwrap_or(0), header("x-height").unwrap_or(0));
    if (w as usize) * (h as usize) * 4 != bytes.len() {
        return Err("标注层尺寸与数据长度不符".into());
    }
    let at = (header("x-x").unwrap_or(0), header("x-y").unwrap_or(0));
    let layer = snapmind_core::export::RgbaImage { width: w, height: h, data: bytes.clone() };
    crate::pin::set_annotations(&app, id, Some((at, layer)));
    Ok(())
}

/// 贴图历史（最近关闭的在前），query 按识别文字搜索。M10.3 的历史页面使用。
#[tauri::command]
pub fn history_pins(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<snapmind_core::history::PinRecord>, String> {
    let history = state.history.as_ref().ok_or("历史记录不可用")?;
    history.list_pins(query.as_deref(), limit.unwrap_or(200)).map_err(|e| e.to_string())
}

/// 四套默认提示词（设置页显示、判断是否改过、恢复默认都以这里为准）。
#[tauri::command]
pub fn prompt_defaults() -> std::collections::HashMap<snapmind_core::prompts::PromptKind, &'static str> {
    snapmind_core::prompts::PromptKind::ALL
        .into_iter()
        .map(|k| (k, snapmind_core::prompts::default_prompt(k)))
        .collect()
}

/// 支持的语言（代码和名字）。
#[tauri::command]
pub fn languages() -> Vec<snapmind_core::lang::Lang> {
    snapmind_core::lang::LANGS.to_vec()
}

/// 翻译历史（最近的在前），query 在原文和译文里搜索。M10.3 的历史页面使用。
#[tauri::command]
pub fn history_translations(
    state: State<'_, AppState>,
    query: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<snapmind_core::history::TransRecord>, String> {
    let history = state.history.as_ref().ok_or("历史记录不可用")?;
    history.list_translations(query.as_deref(), limit.unwrap_or(200)).map_err(|e| e.to_string())
}

/// 删除一条翻译历史。
#[tauri::command]
pub fn history_delete_translation(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    let history = state.history.as_ref().ok_or("历史记录不可用")?;
    history.delete_translation(id).map_err(|e| e.to_string())
}
