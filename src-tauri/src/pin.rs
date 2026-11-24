//! 贴图窗口：把截图钉在屏幕上（需求第 6 节）。
//!
//! - 每张贴图一个窗口 `pin-<id>`：透明、无边框、置顶、不进任务栏；图片四周留一圈透明边距画阴影。
//! - 图片按 1:1 物理像素出现在原来截图的位置；图片数据存在内存里，经 `snapframe` 协议的
//!   `/pin/<id>` 提供给页面（BMP，无需编码）。
//! - 缩放由 Rust 改窗口大小和位置（以鼠标为中心）；当前缩放比例总是从窗口实际尺寸推算，
//!   拖到不同缩放的显示器上被系统改了尺寸也不会错乱。
//! - 鼠标穿透后窗口收不到按键，所以 `Ctrl+Shift+X` 注册为全局快捷键（有贴图时才注册）：
//!   焦点在某张贴图上时让它穿透，否则取消所有贴图的穿透。
//! - 崩溃恢复（M11.2）：开着的贴图在 `配置目录/session` 里留一份（图片 + 位置），关闭时删掉；
//!   托盘退出时全部写进历史再清空。下次启动时还留着的，说明上次没正常退出，按原位置贴回来。

use snapmind_core::i18n::{tr};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use snapmind_core::export::RgbaImage;
use snapmind_core::history::{NewPin, PinSource};
use snapmind_core::session::{PinMeta, Session};
use snapmind_ocr::{layout, Line};
use snapmind_platform::bmp::Frame;
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl, WebviewWindowBuilder};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub const LABEL_PREFIX: &str = "pin-";
/// 鼠标穿透的开关键。
pub const THROUGH_SHORTCUT: &str = "Ctrl+Shift+X";
/// 图片四周的透明边距（CSS 像素），用来画阴影。页面里有同一个常量。
pub const MARGIN_CSS: f64 = 10.0;
/// 缩放范围。
pub const ZOOM_MIN: f64 = 0.1;
pub const ZOOM_MAX: f64 = 8.0;

pub struct Pin {
    /// 供页面显示的 BMP
    pub frame: Arc<Frame>,
    /// 原始像素（复制、保存、识别用）
    pub image: Arc<RgbaImage>,
    pub through: bool,
    /// 文字卡片的原文（贴剪贴板里的文字时）；复制时复制文字
    pub text: Option<String>,
    /// 贴图上的标注层（相对图片左上角的偏移 + RGBA），复制 / 保存时叠到图片上
    pub annotations: Option<((u32, u32), Arc<RgbaImage>)>,
    /// 贴出时间（Unix 毫秒）和来源，关闭时写进历史
    pub created_at: i64,
    pub source: PinSource,
    /// 贴出后后台自动识别的结果（M4.5）：页面据此铺透明文字层；关闭时识别文字写进历史
    pub ocr: Option<Arc<Vec<Line>>>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[derive(Default)]
pub struct PinState {
    next: AtomicU32,
    pins: Mutex<HashMap<u32, Pin>>,
}

#[derive(Debug, Serialize)]
pub struct PinInfo {
    pub width: u32,
    pub height: u32,
    /// 文字卡片：页面负责把文字排版成卡片图片再交回来
    pub text: Option<String>,
}

pub fn label(id: u32) -> String {
    format!("{LABEL_PREFIX}{id}")
}

pub fn id_of(label: &str) -> Option<u32> {
    label.strip_prefix(LABEL_PREFIX)?.parse().ok()
}

/// 由当前窗口宽度推算缩放比例：(窗口宽 − 两侧边距) / 图片宽。
pub fn zoom_of(window_width: u32, margin_phys: u32, image_width: u32) -> f64 {
    (window_width.saturating_sub(2 * margin_phys)) as f64 / image_width.max(1) as f64
}

/// 以锚点（窗口内物理像素）为中心缩放后，窗口新的位置和大小。
pub fn zoom_geometry(
    pos: (i32, i32),
    margin: u32,
    image: (u32, u32),
    old_zoom: f64,
    new_zoom: f64,
    anchor: (f64, f64),
) -> ((i32, i32), (u32, u32)) {
    let w = (image.0 as f64 * new_zoom).round().max(1.0) as u32 + 2 * margin;
    let h = (image.1 as f64 * new_zoom).round().max(1.0) as u32 + 2 * margin;
    // 锚点相对图片左上角的位置按比例缩放，保持锚点在屏幕上不动
    let k = new_zoom / old_zoom;
    let ax = anchor.0 - margin as f64;
    let ay = anchor.1 - margin as f64;
    let x = pos.0 as f64 + ax - ax * k;
    let y = pos.1 as f64 + ay - ay * k;
    ((x.round() as i32, y.round() as i32), (w, h))
}

fn rgba_to_bgra(img: &RgbaImage) -> Vec<u8> {
    img.data.chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], p[3]]).collect()
}

/// 新建贴图。`at` 是图片左上角的虚拟桌面物理坐标，`scale` 是所在显示器的缩放。
pub fn create<R: Runtime>(
    app: &AppHandle<R>,
    image: RgbaImage,
    source: PinSource,
    at: (i32, i32),
    scale: f64,
) -> tauri::Result<u32> {
    create_with(app, image, None, source, at, scale)
}

/// 新建贴图并按 `zoom` 缩小显示（长截图：图片比屏幕还长）。
pub fn create_zoomed<R: Runtime>(app: &AppHandle<R>, image: RgbaImage, at: (i32, i32), scale: f64, zoom: f64) -> tauri::Result<u32> {
    let (w, h) = (image.width, image.height);
    let id = create_with(app, image, None, PinSource::Capture, at, scale)?;
    if zoom < 1.0 {
        if let Some(win) = app.get_webview_window(&label(id)) {
            let margin = (MARGIN_CSS * scale).round() as u32;
            let size = PhysicalSize::new(
                (w as f64 * zoom).round().max(1.0) as u32 + 2 * margin,
                (h as f64 * zoom).round().max(1.0) as u32 + 2 * margin,
            );
            let _ = win.set_size(size);
        }
    }
    Ok(id)
}

/// 新建文字卡片贴图：先用占位图建窗口，页面按主题把文字排成卡片后通过 [`set_image`] 交回。
pub fn create_text<R: Runtime>(app: &AppHandle<R>, text: String, at: (i32, i32), scale: f64) -> tauri::Result<u32> {
    let placeholder = RgbaImage { width: 1, height: 1, data: vec![0, 0, 0, 0] };
    create_with(app, placeholder, Some(text), PinSource::Text, at, scale)
}

fn create_with<R: Runtime>(
    app: &AppHandle<R>,
    image: RgbaImage,
    text: Option<String>,
    source: PinSource,
    at: (i32, i32),
    scale: f64,
) -> tauri::Result<u32> {
    let state = app.state::<PinState>();
    let id = state.next.fetch_add(1, Ordering::SeqCst) + 1;
    let (w, h) = (image.width, image.height);
    let frame = Arc::new(Frame::from_bgra(w, h, &rgba_to_bgra(&image)));
    state
        .pins
        .lock()
        .expect("贴图锁")
        .insert(id, Pin {
            frame,
            image: Arc::new(image),
            through: false,
            text,
            annotations: None,
            created_at: now_ms(),
            source,
            ocr: None,
        });

    let margin = (MARGIN_CSS * scale).round() as i32;
    let win = WebviewWindowBuilder::new(app, label(id), WebviewUrl::App("pin/index.html".into()))
        .title(tr("SnapMind 贴图"))
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .visible(false)
        .additional_browser_args(crate::overlay::BROWSER_ARGS)
        .build()?;
    let _ = win.set_position(PhysicalPosition::new(at.0 - margin, at.1 - margin));
    let _ = win.set_size(PhysicalSize::new(w + 2 * margin as u32, h + 2 * margin as u32));
    if let Some(h) = crate::wm::native_handle(&win) {
        snapmind_platform::window::disable_transitions(h);
    }
    ensure_shortcut(app);
    remember(app, id);
    // 截图贴图的图片在后台写一份；文字卡片按原文恢复，不用存图
    if !is_text(app, id) {
        save_image_later(app, id);
    }
    Ok(id)
}

fn session<R: Runtime>(app: &AppHandle<R>) -> Option<Session> {
    crate::state::config_dir(app).ok().map(|d| Session::new(&d.join("session")))
}

fn is_text<R: Runtime>(app: &AppHandle<R>, id: u32) -> bool {
    app.state::<PinState>().pins.lock().expect("贴图锁").get(&id).is_some_and(|p| p.text.is_some())
}

/// 记下贴图的描述和当前窗口位置（新建、拖动、缩放后调用；文件很小，直接写）。
pub fn remember<R: Runtime>(app: &AppHandle<R>, id: u32) {
    let Some(win) = app.get_webview_window(&label(id)) else { return };
    let (Ok(pos), Ok(size)) = (win.outer_position(), win.outer_size()) else { return };
    let meta = {
        let state = app.state::<PinState>();
        let pins = state.pins.lock().expect("贴图锁");
        let Some(p) = pins.get(&id) else { return };
        PinMeta {
            source: p.source,
            text: p.text.clone(),
            created_at: p.created_at,
            rect: (pos.x, pos.y, size.width, size.height),
            scale: win.scale_factor().unwrap_or(1.0),
        }
    };
    if let Some(s) = session(app) {
        if let Err(e) = s.save_meta(id, &meta) {
            crate::log_warn!("记录贴图 {id} 失败：{e}");
        }
    }
}

/// 后台把贴图当前的样子（带标注）写一份。写完时贴图已经关了就删掉，免得留下残余。
fn save_image_later<R: Runtime>(app: &AppHandle<R>, id: u32) {
    let app = app.clone();
    std::thread::spawn(move || {
        let (Some(img), Some(s)) = (image(&app, id), session(&app)) else { return };
        if let Err(e) = s.save_image(id, &img) {
            crate::log_warn!("记录贴图 {id} 的图片失败：{e}");
        }
        if !app.state::<PinState>().pins.lock().expect("贴图锁").contains_key(&id) {
            s.remove(id);
        }
    });
}

/// 上次没正常退出时留下的贴图：按原来的位置和大小贴回来。启动时在后台线程调用。
pub fn restore<R: Runtime>(app: &AppHandle<R>) {
    let Some(s) = session(app) else { return };
    let left = s.take_all();
    if left.is_empty() {
        return;
    }
    crate::log_info!("上次没有正常退出，恢复 {} 张贴图", left.len());
    for l in left {
        let m = l.meta;
        let margin = (MARGIN_CSS * m.scale).round() as i32;
        let at = (m.rect.0 + margin, m.rect.1 + margin);
        let made = match (l.image, m.text) {
            (None, Some(text)) => create_text(app, text, at, m.scale),
            (Some(img), text) => create_with(app, img, text, m.source, at, m.scale),
            (None, None) => continue,
        };
        match made {
            Ok(id) => {
                if let Some(p) = app.state::<PinState>().pins.lock().expect("贴图锁").get_mut(&id) {
                    p.created_at = m.created_at;
                }
                // 缩放过的贴图恢复原来的大小（文字卡片排好版后会按卡片大小再定）
                if let Some(win) = app.get_webview_window(&label(id)) {
                    let _ = win.set_size(PhysicalSize::new(m.rect.2, m.rect.3));
                }
                remember(app, id);
            }
            Err(e) => crate::log_warn!("恢复贴图失败：{e}"),
        }
    }
}

/// 页面画好后显示窗口并给它焦点（避免透明窗口先闪一下空白）。
pub fn ready<R: Runtime>(app: &AppHandle<R>, id: u32) {
    if let Some(w) = app.get_webview_window(&label(id)) {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

pub fn info<R: Runtime>(app: &AppHandle<R>, id: u32) -> Option<PinInfo> {
    let state = app.state::<PinState>();
    let pins = state.pins.lock().expect("贴图锁");
    pins.get(&id).map(|p| PinInfo { width: p.image.width, height: p.image.height, text: p.text.clone() })
}

/// 换掉贴图的图片（文字卡片排版完成后），窗口按新尺寸以 100% 显示、左上角不动。
pub fn set_image<R: Runtime>(app: &AppHandle<R>, id: u32, image: RgbaImage) {
    let (w, h) = (image.width, image.height);
    {
        let state = app.state::<PinState>();
        let mut pins = state.pins.lock().expect("贴图锁");
        let Some(p) = pins.get_mut(&id) else { return };
        p.frame = Arc::new(Frame::from_bgra(w, h, &rgba_to_bgra(&image)));
        p.image = Arc::new(image);
    }
    if let Some(win) = app.get_webview_window(&label(id)) {
        let scale = win.scale_factor().unwrap_or(1.0);
        let margin = (MARGIN_CSS * scale).round() as u32;
        let _ = win.set_size(PhysicalSize::new(w + 2 * margin, h + 2 * margin));
    }
}

/// 设置（None 为清除）贴图的标注层。
pub fn set_annotations<R: Runtime>(app: &AppHandle<R>, id: u32, layer: Option<((u32, u32), RgbaImage)>) {
    if let Some(p) = app.state::<PinState>().pins.lock().expect("贴图锁").get_mut(&id) {
        p.annotations = layer.map(|(at, l)| (at, Arc::new(l)));
    }
    if !is_text(app, id) {
        save_image_later(app, id);
    }
}

/// 复制贴图：没有标注的文字卡片复制原文；其他情况复制图片（位图 + PNG + 文件，带标注）。
pub fn copy<R: Runtime>(app: &AppHandle<R>, id: u32, template: &str) -> Result<(), String> {
    let (text, has_annotations) = {
        let state = app.state::<PinState>();
        let pins = state.pins.lock().expect("贴图锁");
        let p = pins.get(&id).ok_or_else(|| tr("贴图已关闭"))?;
        (p.text.clone(), p.annotations.is_some())
    };
    match text {
        Some(t) if !has_annotations => snapmind_platform::clipboard::set_text(&t).map_err(|e| e.to_string()),
        _ => {
            let img = image(app, id).ok_or_else(|| tr("贴图已关闭"))?;
            crate::export::copy_image(&img, template).map(|_| ())
        }
    }
}

/// 贴图的原图（不带标注）和是否是文字卡片：自动识别用。
pub fn base_image<R: Runtime>(app: &AppHandle<R>, id: u32) -> Option<(Arc<RgbaImage>, bool)> {
    let state = app.state::<PinState>();
    let pins = state.pins.lock().expect("贴图锁");
    pins.get(&id).map(|p| (p.image.clone(), p.text.is_some()))
}

pub fn set_ocr<R: Runtime>(app: &AppHandle<R>, id: u32, lines: Arc<Vec<Line>>) {
    if let Some(p) = app.state::<PinState>().pins.lock().expect("贴图锁").get_mut(&id) {
        p.ocr = Some(lines);
    }
}

pub fn ocr<R: Runtime>(app: &AppHandle<R>, id: u32) -> Option<Arc<Vec<Line>>> {
    app.state::<PinState>().pins.lock().expect("贴图锁").get(&id).and_then(|p| p.ocr.clone())
}

/// 贴图的图片，有标注时按原始分辨率叠上标注（复制、保存、历史用）。
pub fn image<R: Runtime>(app: &AppHandle<R>, id: u32) -> Option<Arc<RgbaImage>> {
    let (base, ann) = {
        let state = app.state::<PinState>();
        let pins = state.pins.lock().expect("贴图锁");
        let p = pins.get(&id)?;
        (p.image.clone(), p.annotations.clone())
    };
    let Some(((dx, dy), layer)) = ann else { return Some(base) };
    let mut img = (*base).clone();
    match snapmind_core::export::composite_over(&mut img, &layer, dx, dy) {
        Ok(()) => Some(Arc::new(img)),
        Err(_) => Some(base), // 标注层与图片尺寸不符（不应发生），退回原图
    }
}

pub fn frame<R: Runtime>(app: &AppHandle<R>, id: u32) -> Option<Arc<Frame>> {
    let state = app.state::<PinState>();
    let pins = state.pins.lock().expect("贴图锁");
    pins.get(&id).map(|p| p.frame.clone())
}

/// 缩放。`factor` 为相对倍数（滚轮）；`absolute` 为目标比例（Ctrl+0 → 1.0）。
/// 锚点是窗口内的 CSS 坐标。返回缩放后的比例。
pub fn zoom<R: Runtime>(
    app: &AppHandle<R>,
    id: u32,
    factor: Option<f64>,
    absolute: Option<f64>,
    anchor_css: (f64, f64),
) -> Option<f64> {
    let image = info(app, id)?;
    let w = app.get_webview_window(&label(id))?;
    let scale = w.scale_factor().ok()?;
    let margin = (MARGIN_CSS * scale).round() as u32;
    let size = w.inner_size().ok()?;
    let pos = w.outer_position().ok()?;
    let old = zoom_of(size.width, margin, image.width);
    let target = absolute.unwrap_or(old * factor.unwrap_or(1.0)).clamp(ZOOM_MIN, ZOOM_MAX);
    if (target - old).abs() < 1e-4 {
        return Some(old);
    }
    let anchor = (anchor_css.0 * scale, anchor_css.1 * scale);
    let (p, s) = zoom_geometry((pos.x, pos.y), margin, (image.width, image.height), old, target, anchor);
    let _ = w.set_position(PhysicalPosition::new(p.0, p.1));
    let _ = w.set_size(PhysicalSize::new(s.0, s.1));
    Some(target)
}

/// 设置一张贴图的鼠标穿透，并同步托盘菜单。
pub fn set_through<R: Runtime>(app: &AppHandle<R>, id: u32, on: bool) {
    if let Some(w) = app.get_webview_window(&label(id)) {
        let _ = w.set_ignore_cursor_events(on);
        let _ = tauri::Emitter::emit_to(app, label(id), "pin:through", on);
    }
    let any = {
        let state = app.state::<PinState>();
        let mut pins = state.pins.lock().expect("贴图锁");
        if let Some(p) = pins.get_mut(&id) {
            p.through = on;
        }
        pins.values().any(|p| p.through)
    };
    crate::tray::sync_through(app, any);
}

/// 取消所有贴图的鼠标穿透（全局快捷键、托盘菜单）。
pub fn clear_through<R: Runtime>(app: &AppHandle<R>) {
    let ids: Vec<u32> = {
        let state = app.state::<PinState>();
        let pins = state.pins.lock().expect("贴图锁");
        pins.iter().filter(|(_, p)| p.through).map(|(id, _)| *id).collect()
    };
    for id in ids {
        set_through(app, id, false);
    }
}

/// 全局 Ctrl+Shift+X：焦点在某张贴图上 → 让它穿透；否则 → 取消所有贴图的穿透。
pub fn toggle_through<R: Runtime>(app: &AppHandle<R>) {
    let focused = app
        .webview_windows()
        .into_iter()
        .filter_map(|(l, w)| id_of(&l).filter(|_| w.is_focused().unwrap_or(false)))
        .next();
    match focused {
        Some(id) => set_through(app, id, true),
        None => clear_through(app),
    }
}

/// 关闭贴图：写入历史（后台线程编码图片）、销毁窗口、释放图片；没有贴图了就注销穿透快捷键。
pub fn close<R: Runtime>(app: &AppHandle<R>, id: u32) {
    // 先取出带标注的图片，再从表里移除
    let image = image(app, id);
    let (removed, empty, any_through) = {
        let state = app.state::<PinState>();
        let mut pins = state.pins.lock().expect("贴图锁");
        let removed = pins.remove(&id);
        (removed, pins.is_empty(), pins.values().any(|p| p.through))
    };
    if let Some(s) = session(app) {
        s.remove(id);
    }
    if let (Some(pin), Some(image)) = (removed, image) {
        let app = app.clone();
        std::thread::spawn(move || {
            record(&app, &pin, &image);
            crate::history::prune(&app);
        });
    }
    crate::tray::sync_through(app, any_through);
    crate::pintools::forget(app, id);
    if let Some(w) = app.get_webview_window(&label(id)) {
        let _ = w.destroy();
    }
    if empty {
        let _ = app.global_shortcut().unregister(THROUGH_SHORTCUT);
    }
}

/// 关掉的贴图写进历史。文字卡片排版前（1×1 占位图）就被关掉的不记。
fn record<R: Runtime>(app: &AppHandle<R>, pin: &Pin, image: &RgbaImage) {
    if image.width <= 1 && image.height <= 1 {
        return;
    }
    let state = app.state::<crate::state::AppState>();
    let Some(history) = state.history.as_ref() else { return };
    // 文字卡片记原文，截图贴图记自动识别出的文字（历史里可以按文字搜索）
    let recognized = pin.ocr.as_ref().map(|l| layout::compose(l, &(0..l.len()).collect::<Vec<_>>(), false));
    let rec = NewPin {
        image,
        text: pin.text.as_deref().or(recognized.as_deref()).unwrap_or(""),
        source: pin.source,
        created_at: pin.created_at,
        closed_at: now_ms(),
    };
    if let Err(e) = history.add_pin(rec) {
        crate::log_warn!("贴图写入历史失败：{e}");
    }
}

/// 托盘退出：开着的贴图都当作关闭写进历史（同步写完再退出），然后清掉崩溃恢复用的那份。
pub fn close_all_for_exit<R: Runtime>(app: &AppHandle<R>) {
    let ids: Vec<u32> = app.state::<PinState>().pins.lock().expect("贴图锁").keys().copied().collect();
    for id in ids {
        let image = image(app, id);
        let pin = app.state::<PinState>().pins.lock().expect("贴图锁").remove(&id);
        if let (Some(pin), Some(image)) = (pin, image) {
            record(app, &pin, &image);
        }
    }
    crate::history::prune(app);
    if let Some(s) = session(app) {
        s.clear();
    }
}

fn ensure_shortcut<R: Runtime>(app: &AppHandle<R>) {
    let gs = app.global_shortcut();
    if !gs.is_registered(THROUGH_SHORTCUT) {
        if let Err(e) = gs.register(THROUGH_SHORTCUT) {
            // 被别的程序占用：贴图窗口自己处理按键开启穿透，托盘菜单负责取消
            crate::log_warn!("注册穿透快捷键 {THROUGH_SHORTCUT} 失败（改由贴图窗口和托盘菜单处理）：{e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        assert_eq!(label(3), "pin-3");
        assert_eq!(id_of("pin-12"), Some(12));
        assert_eq!(id_of("overlay-0"), None);
    }

    #[test]
    fn zoom_from_window_size() {
        assert_eq!(zoom_of(520, 10, 500), 1.0);
        assert_eq!(zoom_of(1020, 10, 500), 2.0);
    }

    #[test]
    fn zoom_keeps_anchor_fixed() {
        // 500×300 的图片，边距 10，窗口在 (100,100)；在图片中心 (260,160) 处放大到 2 倍
        let ((x, y), (w, h)) = zoom_geometry((100, 100), 10, (500, 300), 1.0, 2.0, (260.0, 160.0));
        assert_eq!((w, h), (1020, 620));
        // 锚点原来在屏幕 (360,260)，放大后仍在同一位置
        assert_eq!((x + 10 + 500, y + 10 + 300), (360, 260));
    }

    #[test]
    fn zoom_at_top_left_corner_of_image() {
        let ((x, y), _) = zoom_geometry((0, 0), 10, (100, 100), 1.0, 3.0, (10.0, 10.0));
        assert_eq!((x, y), (0, 0), "锚点在图片左上角时窗口不动");
    }
}
