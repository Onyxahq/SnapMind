//! 截图的「交出去」动作：复制、保存、估算文件大小、系统对话框。
//!
//! 都从当前截图会话的冻结帧里裁剪（本屏物理像素），所以必须在结束截图前调用；
//! 复制和保存成功后会结束截图（需求 3.1：把结果交出去的动作都会结束截图）。

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

/// 从会话里的冻结帧裁出选区。
fn crop<R: Runtime>(app: &AppHandle<R>, index: usize, r: Region) -> Result<RgbaImage, String> {
    let frame = overlay::frame(app, index).ok_or("截图已结束")?;
    export::crop_bgra(frame.pixels(), frame.width, frame.height, r.x, r.y, r.width, r.height).map_err(err)
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
    tauri::async_runtime::spawn_blocking(move || {
        let png = export::encode(&img, ImageFormat::Png, 0, true).map_err(err)?;
        let dir = clip_dir();
        std::fs::create_dir_all(&dir).map_err(err)?;
        let stem = export::render_filename(&template, export::now_local());
        let path = export::unique_path(&dir, &stem, "png");
        std::fs::write(&path, &png).map_err(err)?;
        let clip = ImageClip { width: img.width, height: img.height, rgba: &img.data, png: &png, file: Some(&path) };
        clipboard::set_image(&clip).map_err(err)?;
        Ok(path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default())
    })
    .await
    .map_err(err)?
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
    add("桌面", p.desktop_dir().ok());
    add("图片", p.picture_dir().ok());
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
