//! 截图的「交出去」动作：复制、保存、估算文件大小、系统对话框。
//!
//! 都从当前截图会话的冻结帧里裁剪（本屏物理像素），所以必须在结束截图前调用；
//! 复制和保存成功后会结束截图（需求 3.1：把结果交出去的动作都会结束截图）。

use snapmind_core::i18n::{tr, trf};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use snapmind_core::config::Config;
use snapmind_core::export::{self, ExportError, ImageFormat, RgbaImage};
use snapmind_platform::clipboard::{self, ImageClip};
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::DialogExt;

use crate::overlay;
use crate::state::AppState;

/// 前端传来的选区（本屏物理像素）。
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Region {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// 输出选项：格式、质量、输出尺寸（与选区不同时缩放）。
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Output {
    pub format: ImageFormat,
    pub quality: u8,
    pub width: u32,
    pub height: u32,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

/// 从会话里的冻结帧裁出选区原图（不带标注），识别和翻译用。
pub fn crop_base<R: Runtime>(app: &AppHandle<R>, index: usize, r: Region) -> Result<RgbaImage, String> {
    let frame = overlay::frame(app, index).ok_or_else(|| tr("截图已结束"))?;
    export::crop_bgra(frame.pixels(), frame.width, frame.height, r.x, r.y, r.width, r.height).map_err(err)
}

/// 把文字贴成文字卡片（翻译卡片里的「贴图」），放在选区左上角，然后结束截图。
/// 必须是 async 命令：同步命令跑在主线程上，在里面建窗口会卡死。
#[tauri::command]
pub async fn capture_pin_text(app: AppHandle, index: usize, region: Region, text: String) -> Result<u32, String> {
    let mon = overlay::monitor(&app, index).ok_or_else(|| tr("截图已结束"))?;
    overlay::end(&app);
    let at = (mon.x + region.x as i32, mon.y + region.y as i32);
    crate::pin::create_text(&app, text, at, mon.scale).map_err(err)
}

/// 从会话里的冻结帧裁出选区，有标注时按原始分辨率叠上标注层。
fn crop<R: Runtime>(app: &AppHandle<R>, index: usize, r: Region) -> Result<RgbaImage, String> {
    let frame = overlay::frame(app, index).ok_or_else(|| tr("截图已结束"))?;
    let mut img =
        export::crop_bgra(frame.pixels(), frame.width, frame.height, r.x, r.y, r.width, r.height).map_err(err)?;
    if let Some(((dx, dy), layer)) = overlay::annotations(app, index) {
        // 超出选区说明上传的标注层已过时（选区又变了），不能错位叠加
        export::composite_over(&mut img, &layer, dx, dy).map_err(|_| tr("标注层超出选区"))?;
    }
    Ok(img)
}

/// 上传标注层：请求体是 RGBA 像素（为空表示清除）；请求头 `x-index` 是显示器序号，
/// `x-x` / `x-y` 是相对选区左上角的偏移，`x-width` / `x-height` 是尺寸。二进制传输，不经过 JSON。
#[tauri::command]
pub fn capture_set_annotations(app: AppHandle, request: tauri::ipc::Request<'_>) -> Result<(), String> {
    let tauri::ipc::InvokeBody::Raw(bytes) = request.body() else {
        return Err(tr("标注层需要以二进制上传"));
    };
    let header = |name: &str| {
        request.headers().get(name).and_then(|v| v.to_str().ok()).and_then(|s| s.parse::<u32>().ok())
    };
    let index = header("x-index").ok_or_else(|| tr("缺少 x-index"))? as usize;
    if bytes.is_empty() {
        overlay::set_annotations(&app, index, None);
        return Ok(());
    }
    let (width, height) = (header("x-width").unwrap_or(0), header("x-height").unwrap_or(0));
    if width as usize * height as usize * 4 != bytes.len() {
        return Err(tr("标注层尺寸与数据长度不符"));
    }
    let at = (header("x-x").unwrap_or(0), header("x-y").unwrap_or(0));
    overlay::set_annotations(&app, index, Some((at, RgbaImage { width, height, data: bytes.clone() })));
    Ok(())
}

fn render(img: RgbaImage, out: Output, fast: bool) -> Result<Vec<u8>, ExportError> {
    let img = export::resize(img, out.width, out.height)?;
    export::encode(&img, out.format, out.quality, fast)
}

/// 默认保存目录：配置里的目录，或者「图片\SnapMind」。
pub fn default_dir<R: Runtime>(app: &AppHandle<R>, config: &Config) -> PathBuf {
    if !config.save.dir.trim().is_empty() {
        return PathBuf::from(config.save.dir.trim());
    }
    let pictures = app.path().picture_dir().unwrap_or_else(|_| std::env::temp_dir());
    pictures.join(snapmind_core::APP_NAME)
}

/// 剪贴板临时文件目录：%TEMP%\SnapMind。
fn clip_dir() -> PathBuf {
    std::env::temp_dir().join(snapmind_core::APP_NAME)
}

/// 清理一天前的剪贴板临时文件（启动时调用）。
pub fn cleanup_clip_files() {
    let Ok(entries) = std::fs::read_dir(clip_dir()) else { return };
    let day = std::time::Duration::from_secs(24 * 3600);
    for e in entries.flatten() {
        let old = e
            .metadata()
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_some_and(|age| age > day);
        if old {
            let _ = std::fs::remove_file(e.path());
        }
    }
}

/// 复制：位图 + PNG + 临时 PNG 文件一起放进剪贴板，然后结束截图。返回临时文件名。
#[tauri::command]
pub async fn capture_copy(app: AppHandle, state: State<'_, AppState>, index: usize, region: Region) -> Result<String, String> {
    let img = crop(&app, index, region)?;
    overlay::end(&app); // 先让遮罩消失，编码和写剪贴板在后台做
    let template = state.config.get().save.filename_template;
    tauri::async_runtime::spawn_blocking(move || copy_image(&img, &template)).await.map_err(err)?
}

/// 把图片放进剪贴板：位图 + PNG + 按模板命名的临时 PNG 文件（截图和贴图共用）。返回文件名。
pub fn copy_image(img: &RgbaImage, template: &str) -> Result<String, String> {
    let png = export::encode(img, ImageFormat::Png, 0, true).map_err(err)?;
    let dir = clip_dir();
    std::fs::create_dir_all(&dir).map_err(err)?;
    let stem = export::render_filename(template, export::now_local());
    let path = export::unique_path(&dir, &stem, "png");
    std::fs::write(&path, &png).map_err(err)?;
    let clip = ImageClip { width: img.width, height: img.height, rgba: &img.data, png: &png, file: Some(&path) };
    clipboard::set_image(&clip).map_err(err)?;
    Ok(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
}

/// 贴图：裁剪（带标注）后结束截图，在原位置钉一张贴图。返回贴图 id。
#[tauri::command]
pub async fn capture_pin(app: AppHandle, index: usize, region: Region) -> Result<u32, String> {
    let img = crop(&app, index, region)?;
    let mon = overlay::monitor(&app, index).ok_or_else(|| tr("截图已结束"))?;
    overlay::end(&app);
    let at = (mon.x + region.x as i32, mon.y + region.y as i32);
    crate::pin::create(&app, img, snapmind_core::history::PinSource::Capture, at, mon.scale).map_err(err)
}

/// 保存到哪里：
/// - `path`：系统另存为对话框选的完整路径（对话框已确认过覆盖）；
/// - `dir` + `name`：SnapMind 保存窗口里填的目录和文件名，重名时自动加 (2)，不覆盖；
/// - 都没有：按模板存到默认目录（Ctrl+S 设为「直接保存」时）。
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Target {
    pub path: Option<String>,
    pub dir: Option<String>,
    pub name: Option<String>,
}

/// 保存，然后结束截图。返回实际保存的路径。
#[tauri::command]
pub async fn capture_save(
    app: AppHandle,
    state: State<'_, AppState>,
    index: usize,
    region: Region,
    output: Output,
    target: Target,
) -> Result<String, String> {
    let img = crop(&app, index, region)?;
    let config = state.config.get();
    let nonempty = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let ext = output.format.extension();
    let path = if let Some(p) = nonempty(target.path) {
        PathBuf::from(p)
    } else {
        let dir = nonempty(target.dir).map(PathBuf::from).unwrap_or_else(|| default_dir(&app, &config));
        let stem = nonempty(target.name)
            .map(|n| export::sanitize_filename(&n))
            .unwrap_or_else(|| export::render_filename(&config.save.filename_template, export::now_local()));
        export::unique_path(&dir, &stem, ext)
    };
    overlay::end(&app);
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = render(img, output, false).map_err(err)?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(err)?;
        }
        std::fs::write(&path, bytes).map_err(err)?;
        Ok(path.to_string_lossy().into_owned())
    })
    .await
    .map_err(err)?
}

/// 估算保存后的文件大小（字节）：真的编码一次，所以是准确值。
#[tauri::command]
pub async fn capture_estimate(app: AppHandle, index: usize, region: Region, output: Output) -> Result<u64, String> {
    let img = crop(&app, index, region)?;
    tauri::async_runtime::spawn_blocking(move || render(img, output, false).map(|b| b.len() as u64).map_err(err))
        .await
        .map_err(err)?
}

#[derive(Debug, Serialize)]
pub struct Location {
    pub label: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveInfo {
    /// 默认目录
    pub dir: String,
    /// 按模板生成的文件名（不含扩展名）
    pub name: String,
    /// 常用位置：桌面、图片、SnapMind，以及用户添加的
    pub quick: Vec<Location>,
}

/// 保存窗口打开时需要的信息。
#[tauri::command]
pub fn save_info(app: AppHandle, state: State<'_, AppState>) -> SaveInfo {
    let config = state.config.get();
    let p = app.path();
    let mut quick = Vec::new();
    let mut add = |label: &str, path: Option<PathBuf>| {
        if let Some(path) = path {
            quick.push(Location { label: label.into(), path: path.to_string_lossy().into_owned() });
        }
    };
    add(&tr("桌面"), p.desktop_dir().ok());
    add(&tr("图片"), p.picture_dir().ok());
    add("SnapMind", p.picture_dir().ok().map(|d| d.join(snapmind_core::APP_NAME)));
    for d in &config.save.extra_dirs {
        let label = Path::new(d).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| d.clone());
        add(&label, Some(PathBuf::from(d)));
    }
    SaveInfo {
        dir: default_dir(&app, &config).to_string_lossy().into_owned(),
        name: export::render_filename(&config.save.filename_template, export::now_local()),
        quick,
    }
}

/// 系统「另存为」对话框，返回选中的路径；取消返回 None。对话框挂在当前遮罩窗口下，显示在遮罩之上。
#[tauri::command]
pub async fn pick_save_file(window: tauri::WebviewWindow, dir: String, name: String, format: ImageFormat) -> Option<String> {
    let ext = format.extension();
    let label = ext.to_uppercase();
    tauri::async_runtime::spawn_blocking(move || {
        window
            .dialog()
            .file()
            .set_parent(&window)
            .set_directory(dir)
            .set_file_name(format!("{name}.{ext}"))
            .add_filter(label, &[ext])
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .ok()
    .flatten()
}

/// 把 Markdown 存成 .md（系统另存为对话框，默认目录和文件名模板与截图一致）。返回保存路径，取消为 None。
#[tauri::command]
pub async fn save_markdown(app: AppHandle, window: tauri::WebviewWindow, text: String) -> Result<Option<String>, String> {
    let config = app.state::<AppState>().config.get();
    let dir = default_dir(&app, &config);
    let name = export::render_filename(&config.save.filename_template, export::now_local());
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let _ = std::fs::create_dir_all(&dir);
        window
            .dialog()
            .file()
            .set_parent(&window)
            .set_directory(dir)
            .set_file_name(format!("{name}.md"))
            .add_filter("Markdown", &["md"])
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = picked else { return Ok(None) };
    std::fs::write(&path, text.as_bytes()).map_err(|e| trf("保存失败：{e}", &[("e", &e.to_string())]))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// 选择文件夹（保存窗口的「＋添加」）。
#[tauri::command]
pub async fn pick_folder(window: tauri::WebviewWindow, dir: String) -> Option<String> {
    tauri::async_runtime::spawn_blocking(move || {
        window
            .dialog()
            .file()
            .set_parent(&window)
            .set_directory(dir)
            .blocking_pick_folder()
            .and_then(|p| p.into_path().ok())
            .map(|p| p.to_string_lossy().into_owned())
    })
    .await
    .ok()
    .flatten()
}

/// 在资源管理器里打开文件夹（不存在时先创建）。
#[tauri::command]
pub fn open_folder(path: String) -> Result<(), String> {
    std::fs::create_dir_all(&path).map_err(err)?;
    std::process::Command::new("explorer").arg(&path).spawn().map_err(err)?;
    Ok(())
}
