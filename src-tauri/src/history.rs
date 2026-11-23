//! 历史记录（需求第 7 节、M10.3）：保留方式、占用统计、重新贴出、复制、删除、清空，
//! 以及给设置页显示缩略图的 `snapthumb` 协议。数据本身在 `snapmind_core::history`。

use snapmind_core::i18n::{tr, trf};
use std::borrow::Cow;

use serde::Serialize;
use snapmind_core::config::{HistoryConfig, HistoryMode};
use snapmind_core::export;
use snapmind_core::history::{History, PinSource, Retention, Usage};
use tauri::http::{Request, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::state::AppState;

/// 自定义协议名：`snapthumb://localhost/<id>` 返回那条贴图历史的缩略图（JPEG）。
pub const PROTOCOL: &str = "snapthumb";

pub fn retention(cfg: &HistoryConfig) -> Retention {
    match cfg.mode {
        HistoryMode::Unlimited => Retention::Unlimited,
        HistoryMode::Count => Retention::Count(cfg.max_count.max(1) as u64),
        HistoryMode::Size => Retention::Bytes(cfg.max_mb.max(1) as u64 * 1024 * 1024),
    }
}

/// 按当前的保留方式清理（写入历史后、改了保留方式后调用；阻塞，放在后台线程里）。
pub fn prune<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let Some(h) = state.history.as_ref() else { return };
    match h.prune(retention(&state.config.get().history)) {
        Ok(r) if r.pins + r.translations > 0 => crate::log_info!("历史记录按保留方式清理：贴图 {} 条，翻译 {} 条", r.pins, r.translations),
        Ok(_) => {}
        Err(e) => crate::log_warn!("清理历史记录失败：{e}"),
    }
}

fn history<'a>(state: &'a State<'_, AppState>) -> Result<&'a History, String> {
    state.history.as_ref().ok_or_else(|| tr("历史记录不可用"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryUsage {
    pub pins: Usage,
    pub translations: Usage,
    /// 历史文件夹
    pub dir: String,
}

#[tauri::command]
pub fn history_usage(state: State<'_, AppState>) -> Result<HistoryUsage, String> {
    let h = history(&state)?;
    Ok(HistoryUsage {
        pins: h.pin_usage().map_err(|e| e.to_string())?,
        translations: h.translation_usage().map_err(|e| e.to_string())?,
        dir: h.dir().to_string_lossy().into_owned(),
    })
}

#[tauri::command]
pub fn history_delete_pin(state: State<'_, AppState>, id: i64) -> Result<bool, String> {
    history(&state)?.delete_pin(id).map_err(|e| e.to_string())
}

/// 清空一类历史（`pins` / `translations`），返回删了几条。
#[tauri::command]
pub fn history_clear(state: State<'_, AppState>, kind: String) -> Result<u64, String> {
    let h = history(&state)?;
    match kind.as_str() {
        "pins" => h.clear_pins(),
        "translations" => h.clear_translations(),
        _ => return Err(trf("不认识的历史类型：{kind}", &[("kind", &kind)])),
    }
    .map_err(|e| e.to_string())
}

/// 鼠标所在显示器：(左上角, 宽高, 缩放)。
fn cursor_monitor<R: Runtime>(app: &AppHandle<R>) -> ((i32, i32), (u32, u32), f64) {
    let cur = app.cursor_position().ok();
    let mons = app.available_monitors().unwrap_or_default();
    let m = mons
        .iter()
        .find(|m| {
            cur.is_some_and(|c| {
                let (p, s) = (m.position(), m.size());
                c.x >= p.x as f64 && c.y >= p.y as f64 && c.x < (p.x + s.width as i32) as f64 && c.y < (p.y + s.height as i32) as f64
            })
        })
        .or(mons.first());
    match m {
        Some(m) => ((m.position().x, m.position().y), (m.size().width, m.size().height), m.scale_factor()),
        None => ((0, 0), (1920, 1080), 1.0),
    }
}

/// 重新贴出一条贴图历史：贴在鼠标所在屏幕的中央（比屏幕大时缩小显示），并从历史里移除
/// （关掉时会重新记进来，不会出现两条）。文字卡片按原文重新排版。返回新贴图的 id。
#[tauri::command]
pub async fn history_repin(app: AppHandle, id: i64) -> Result<u32, String> {
    let rec = {
        let state = app.state::<AppState>();
        history(&state)?.get_pin(id).map_err(|e| e.to_string())?.ok_or_else(|| tr("这条记录已经不在了"))?
    };
    let (origin, size, scale) = cursor_monitor(&app);
    let new_id = if rec.source == PinSource::Text && !rec.text.trim().is_empty() {
        let at = (origin.0 + size.0 as i32 / 3, origin.1 + size.1 as i32 / 3);
        crate::pin::create_text(&app, rec.text.clone(), at, scale).map_err(|e| e.to_string())?
    } else {
        let path = rec.image_path.clone();
        let img = tauri::async_runtime::spawn_blocking(move || {
            let bytes = std::fs::read(&path).map_err(|e| trf("读不到原图：{e}", &[("e", &e.to_string())]))?;
            export::decode_png(&bytes).map_err(|e| e.to_string())
        })
        .await
        .map_err(|e| e.to_string())??;
        let max = ((size.0 as f64 * 0.9) as u32, (size.1 as f64 * 0.9) as u32);
        let zoom = crate::long::fit_zoom((img.width, img.height), max);
        let (w, h) = ((img.width as f64 * zoom) as i32, (img.height as f64 * zoom) as i32);
        let at = (origin.0 + (size.0 as i32 - w) / 2, origin.1 + (size.1 as i32 - h) / 2);
        crate::pin::create_zoomed(&app, img, at, scale, zoom).map_err(|e| e.to_string())?
    };
    let state = app.state::<AppState>();
    if let Ok(h) = history(&state) {
        let _ = h.delete_pin(id);
    }
    Ok(new_id)
}

/// 把一条贴图历史的原图复制到剪贴板。
#[tauri::command]
pub async fn history_copy_pin(app: AppHandle, id: i64) -> Result<(), String> {
    let (path, template) = {
        let state = app.state::<AppState>();
        let rec = history(&state)?.get_pin(id).map_err(|e| e.to_string())?.ok_or_else(|| tr("这条记录已经不在了"))?;
        (rec.image_path, state.config.get().save.filename_template)
    };
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = std::fs::read(&path).map_err(|e| trf("读不到原图：{e}", &[("e", &e.to_string())]))?;
        let img = export::decode_png(&bytes).map_err(|e| e.to_string())?;
        crate::export::copy_image(&img, &template).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// 把一段文字（翻译历史的译文）贴成文字卡片，放在鼠标所在屏幕偏左上。
#[tauri::command]
pub async fn history_pin_text(app: AppHandle, text: String) -> Result<u32, String> {
    let (origin, size, scale) = cursor_monitor(&app);
    let at = (origin.0 + size.0 as i32 / 3, origin.1 + size.1 as i32 / 3);
    crate::pin::create_text(&app, text, at, scale).map_err(|e| e.to_string())
}

/// `snapthumb` 协议：`/<id>` 返回贴图历史的缩略图。
pub fn serve<R: Runtime>(app: &AppHandle<R>, req: &Request<Vec<u8>>) -> Response<Cow<'static, [u8]>> {
    let id = req.uri().path().trim_start_matches('/').parse::<i64>().ok();
    let state = app.state::<AppState>();
    let bytes = match (id, state.history.as_ref()) {
        (Some(id), Some(h)) => std::fs::read(h.dir().join("thumbs").join(format!("{id}.jpg"))).ok(),
        _ => None,
    };
    match bytes {
        Some(b) => Response::builder()
            .header("Content-Type", "image/jpeg")
            .header("Cache-Control", "max-age=31536000, immutable")
            .header("Access-Control-Allow-Origin", "*")
            .body(Cow::Owned(b))
            .expect("响应"),
        None => Response::builder().status(StatusCode::NOT_FOUND).body(Cow::Borrowed(&[][..])).expect("响应"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_retention() {
        let c = |mode, max_count, max_mb| HistoryConfig { mode, max_count, max_mb };
        assert_eq!(retention(&c(HistoryMode::Unlimited, 5, 5)), Retention::Unlimited);
        assert_eq!(retention(&c(HistoryMode::Count, 0, 5)), Retention::Count(1));
        assert_eq!(retention(&c(HistoryMode::Size, 5, 2)), Retention::Bytes(2 * 1024 * 1024));
    }
}
