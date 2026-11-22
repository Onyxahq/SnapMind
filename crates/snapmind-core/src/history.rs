//! 历史记录（需求第 7 节）：贴图历史（M3.4）、翻译历史（M6.4）。
//!
//! 数据放在一个目录里：`history.db`（SQLite）+ `images/<id>.png`（原图，带标注）
//! + `thumbs/<id>.jpg`（缩略图，最长边 320px）。数据库只存元数据和识别文字，
//! 方便按文字搜索；图片放在磁盘上，按空间清理时（M10.3）优先删图片。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::export::{self, ImageFormat, RgbaImage};

/// 缩略图最长边（像素）。
pub const THUMB_MAX: u32 = 320;
/// 数据库结构版本（PRAGMA user_version）。改结构时加一并补迁移。
const SCHEMA_VERSION: i32 = 2;

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("历史数据库出错：{0}")]
    Db(#[from] rusqlite::Error),
    #[error("读写历史文件出错：{0}")]
    Io(#[from] std::io::Error),
    #[error("编码图片失败：{0}")]
    Export(#[from] export::ExportError),
}

/// 贴图从哪里来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PinSource {
    /// 截图里 Ctrl+T 贴出
    Capture,
    /// Shift+F3 贴剪贴板里的图片
    Clipboard,
    /// Shift+F3 贴剪贴板里的文字（文字卡片）
    Text,
}

impl PinSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Clipboard => "clipboard",
            Self::Text => "text",
        }
    }
    fn parse(s: &str) -> Self {
        match s {
            "clipboard" => Self::Clipboard,
            "text" => Self::Text,
            _ => Self::Capture,
        }
    }
}

/// 一条贴图历史。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PinRecord {
    pub id: i64,
    /// 贴出时间、关闭时间（Unix 毫秒）
    pub created_at: i64,
    pub closed_at: i64,
    pub width: u32,
    pub height: u32,
    pub image_path: PathBuf,
    pub thumb_path: PathBuf,
    /// 识别文字（M4 起自动识别）或文字卡片的原文
    pub text: String,
    pub source: PinSource,
    /// 原图 + 缩略图占用的字节数
    pub bytes: u64,
}

/// 新增贴图历史时的输入。
pub struct NewPin<'a> {
    pub image: &'a RgbaImage,
    pub text: &'a str,
    pub source: PinSource,
    pub created_at: i64,
    pub closed_at: i64,
}

/// 占用统计。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Usage {
    pub count: u64,
    pub bytes: u64,
}

/// 翻译从哪里来。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransOrigin {
    /// 截图翻译（Ctrl+Q、Ctrl+Shift+O）
    Capture,
    /// 划词翻译（M7）
    Selection,
}

impl TransOrigin {
    fn as_str(self) -> &'static str {
        match self {
            Self::Capture => "capture",
            Self::Selection => "selection",
        }
    }
    fn parse(s: &str) -> Self {
        if s == "selection" {
            Self::Selection
        } else {
            Self::Capture
        }
    }
}

/// 一条翻译历史。
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransRecord {
    pub id: i64,
    /// 首次翻译、最后一次（重新翻译）的时间，Unix 毫秒
    pub created_at: i64,
    pub updated_at: i64,
    pub source: String,
    pub translation: String,
    /// 检测到的原文语言（统一代码，认不出为空）
    pub source_lang: String,
    pub target_lang: String,
    /// 实际使用的后端和切换说明，如「deepseek-chat 超时，已切换到 DeepL」
    pub backend: String,
    pub origin: TransOrigin,
}

/// 写入 / 更新翻译历史时的输入。
pub struct NewTranslation<'a> {
    pub source: &'a str,
    pub translation: &'a str,
    pub source_lang: &'a str,
    pub target_lang: &'a str,
    pub backend: &'a str,
    pub origin: TransOrigin,
    pub at: i64,
}

pub struct History {
    dir: PathBuf,
    conn: Mutex<Connection>,
}

impl History {
    /// 打开（不存在则创建）历史目录和数据库，并迁移到当前结构。
    pub fn open(dir: &Path) -> Result<Self, HistoryError> {
        fs::create_dir_all(dir.join("images"))?;
        fs::create_dir_all(dir.join("thumbs"))?;
        let conn = Connection::open(dir.join("history.db"))?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        migrate(&conn)?;
        Ok(Self { dir: dir.to_path_buf(), conn: Mutex::new(conn) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// 记一条贴图历史：写原图（PNG）和缩略图（JPEG），再写数据库。返回新记录的 id。
    pub fn add_pin(&self, pin: NewPin) -> Result<PinRecord, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        conn.execute(
            "INSERT INTO pins (created_at, closed_at, width, height, text, source, bytes) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0)",
            params![pin.created_at, pin.closed_at, pin.image.width, pin.image.height, pin.text, pin.source.as_str()],
        )?;
        let id = conn.last_insert_rowid();
        let image_path = self.dir.join("images").join(format!("{id}.png"));
        let thumb_path = self.dir.join("thumbs").join(format!("{id}.jpg"));
        let written = (|| -> Result<u64, HistoryError> {
            let png = export::encode(pin.image, ImageFormat::Png, 0, true)?;
            fs::write(&image_path, &png)?;
            let jpg = export::encode(&thumbnail(pin.image)?, ImageFormat::Jpg, 80, false)?;
            fs::write(&thumb_path, &jpg)?;
            Ok((png.len() + jpg.len()) as u64)
        })();
        let bytes = match written {
            Ok(b) => b,
            Err(e) => {
                // 文件没写成就不留半条记录
                let _ = conn.execute("DELETE FROM pins WHERE id = ?1", [id]);
                let _ = fs::remove_file(&image_path);
                let _ = fs::remove_file(&thumb_path);
                return Err(e);
            }
        };
        conn.execute("UPDATE pins SET bytes = ?1 WHERE id = ?2", params![bytes as i64, id])?;
        Ok(PinRecord {
            id,
            created_at: pin.created_at,
            closed_at: pin.closed_at,
            width: pin.image.width,
            height: pin.image.height,
            image_path,
            thumb_path,
            text: pin.text.to_string(),
            source: pin.source,
            bytes,
        })
    }

    /// 贴图历史，最近关闭的在前；`query` 非空时按识别文字搜索（不区分大小写）。
    pub fn list_pins(&self, query: Option<&str>, limit: usize) -> Result<Vec<PinRecord>, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        let q = query.map(str::trim).filter(|q| !q.is_empty());
        let pattern = q.map(like_pattern);
        let mut stmt = conn.prepare(
            "SELECT id, created_at, closed_at, width, height, text, source, bytes FROM pins
             WHERE (?1 IS NULL OR text LIKE ?1 ESCAPE '\\')
             ORDER BY closed_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], |r| {
            let id: i64 = r.get(0)?;
            Ok(PinRecord {
                id,
                created_at: r.get(1)?,
                closed_at: r.get(2)?,
                width: r.get(3)?,
                height: r.get(4)?,
                image_path: self.dir.join("images").join(format!("{id}.png")),
                thumb_path: self.dir.join("thumbs").join(format!("{id}.jpg")),
                text: r.get(5)?,
                source: PinSource::parse(&r.get::<_, String>(6)?),
                bytes: r.get::<_, i64>(7)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    /// 删除一条贴图历史（连同图片文件）。返回是否存在。
    pub fn delete_pin(&self, id: i64) -> Result<bool, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        let existed = conn.query_row("SELECT 1 FROM pins WHERE id = ?1", [id], |_| Ok(())).optional()?.is_some();
        conn.execute("DELETE FROM pins WHERE id = ?1", [id])?;
        let _ = fs::remove_file(self.dir.join("images").join(format!("{id}.png")));
        let _ = fs::remove_file(self.dir.join("thumbs").join(format!("{id}.jpg")));
        Ok(existed)
    }

    /// 记一条翻译历史，返回 id。
    pub fn add_translation(&self, t: NewTranslation) -> Result<i64, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        conn.execute(
            "INSERT INTO translations (created_at, updated_at, source, translation, source_lang, target_lang, backend, origin)
             VALUES (?1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![t.at, t.source, t.translation, t.source_lang, t.target_lang, t.backend, t.origin.as_str()],
        )?;
        Ok(conn.last_insert_rowid())
    }

    /// 同一张卡片里改了原文重新翻译：更新原记录（不新增一条）。返回是否存在。
    pub fn update_translation(&self, id: i64, t: NewTranslation) -> Result<bool, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        let n = conn.execute(
            "UPDATE translations SET updated_at = ?2, source = ?3, translation = ?4, source_lang = ?5, target_lang = ?6, backend = ?7
             WHERE id = ?1",
            params![id, t.at, t.source, t.translation, t.source_lang, t.target_lang, t.backend],
        )?;
        Ok(n > 0)
    }

    /// 翻译历史，最近的在前；`query` 非空时在原文和译文里搜索（不区分大小写）。
    pub fn list_translations(&self, query: Option<&str>, limit: usize) -> Result<Vec<TransRecord>, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        let q = query.map(str::trim).filter(|q| !q.is_empty());
        let pattern = q.map(like_pattern);
        let mut stmt = conn.prepare(
            "SELECT id, created_at, updated_at, source, translation, source_lang, target_lang, backend, origin FROM translations
             WHERE (?1 IS NULL OR source LIKE ?1 ESCAPE '\\' OR translation LIKE ?1 ESCAPE '\\')
             ORDER BY updated_at DESC, id DESC LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![pattern, limit as i64], |r| {
            Ok(TransRecord {
                id: r.get(0)?,
                created_at: r.get(1)?,
                updated_at: r.get(2)?,
                source: r.get(3)?,
                translation: r.get(4)?,
                source_lang: r.get(5)?,
                target_lang: r.get(6)?,
                backend: r.get(7)?,
                origin: TransOrigin::parse(&r.get::<_, String>(8)?),
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn delete_translation(&self, id: i64) -> Result<bool, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        Ok(conn.execute("DELETE FROM translations WHERE id = ?1", [id])? > 0)
    }

    /// 翻译历史的条数与占用（按文字的 UTF-8 字节数估算）。
    pub fn translation_usage(&self) -> Result<Usage, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        let (count, bytes): (i64, i64) = conn.query_row(
            "SELECT COUNT(*), COALESCE(SUM(LENGTH(CAST(source AS BLOB)) + LENGTH(CAST(translation AS BLOB)) + LENGTH(CAST(backend AS BLOB))), 0)
             FROM translations",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(Usage { count: count as u64, bytes: bytes as u64 })
    }

    /// 贴图历史的条数与占用空间。
    pub fn pin_usage(&self) -> Result<Usage, HistoryError> {
        let conn = self.conn.lock().expect("历史库锁");
        let (count, bytes): (i64, i64) =
            conn.query_row("SELECT COUNT(*), COALESCE(SUM(bytes), 0) FROM pins", [], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(Usage { count: count as u64, bytes: bytes as u64 })
    }
}

/// LIKE 子串匹配的模式（转义 % 和 _）。
fn like_pattern(q: &str) -> String {
    format!("%{}%", q.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_"))
}

/// 缩略图：等比缩到最长边不超过 THUMB_MAX（小图不放大）。
pub fn thumbnail(img: &RgbaImage) -> Result<RgbaImage, export::ExportError> {
    let (w, h) = (img.width, img.height);
    let k = (THUMB_MAX as f64 / w.max(h) as f64).min(1.0);
    let tw = ((w as f64 * k).round() as u32).max(1);
    let th = ((h as f64 * k).round() as u32).max(1);
    export::resize(img.clone(), tw, th)
}

fn migrate(conn: &Connection) -> Result<(), HistoryError> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version < 1 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS pins (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at INTEGER NOT NULL,
                closed_at INTEGER NOT NULL,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                text TEXT NOT NULL DEFAULT '',
                source TEXT NOT NULL,
                bytes INTEGER NOT NULL DEFAULT 0
             );
             CREATE INDEX IF NOT EXISTS pins_closed_at ON pins(closed_at);",
        )?;
    }
    if version < 2 {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS translations (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                translation TEXT NOT NULL,
                source_lang TEXT NOT NULL DEFAULT '',
                target_lang TEXT NOT NULL DEFAULT '',
                backend TEXT NOT NULL DEFAULT '',
                origin TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS translations_updated_at ON translations(updated_at);",
        )?;
    }
    // 以后：if version < 3 { … }
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn img(w: u32, h: u32) -> RgbaImage {
        RgbaImage { width: w, height: h, data: (0..w * h).flat_map(|i| [(i % 251) as u8, 80, 160, 255]).collect() }
    }

    fn add(h: &History, text: &str, closed: i64) -> PinRecord {
        h.add_pin(NewPin { image: &img(640, 360), text, source: PinSource::Capture, created_at: closed - 1000, closed_at: closed })
            .unwrap()
    }

    fn tr<'a>(src: &'a str, dst: &'a str, at: i64) -> NewTranslation<'a> {
        NewTranslation {
            source: src,
            translation: dst,
            source_lang: "en",
            target_lang: "zh-Hans",
            backend: "deepseek-chat 超时，已切换到 DeepL",
            origin: TransOrigin::Capture,
            at,
        }
    }

    #[test]
    fn translations_add_update_search_delete() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        let a = h.add_translation(tr("Hello world", "你好，世界", 1_000)).unwrap();
        let b = h.add_translation(tr("100% sure", "百分之百确定", 2_000)).unwrap();
        let all = h.list_translations(None, 10).unwrap();
        assert_eq!(all.iter().map(|r| r.id).collect::<Vec<_>>(), vec![b, a], "最近的在前");
        assert_eq!(all[1].backend, "deepseek-chat 超时，已切换到 DeepL");
        // 重新翻译：更新同一条，排到最前
        assert!(h.update_translation(a, tr("Hello there", "你好啊", 3_000)).unwrap());
        let all = h.list_translations(None, 10).unwrap();
        assert_eq!((all[0].id, all[0].source.as_str(), all[0].created_at, all[0].updated_at), (a, "Hello there", 1_000, 3_000));
        // 原文、译文都能搜；% 按字面匹配
        assert_eq!(h.list_translations(Some("你好"), 10).unwrap().len(), 1);
        assert_eq!(h.list_translations(Some("HELLO"), 10).unwrap().len(), 1);
        assert_eq!(h.list_translations(Some("%"), 10).unwrap()[0].id, b);
        let u = h.translation_usage().unwrap();
        assert_eq!(u.count, 2);
        assert!(u.bytes > 0);
        assert!(h.delete_translation(b).unwrap());
        assert!(!h.delete_translation(b).unwrap());
        assert!(!h.update_translation(b, tr("x", "y", 4_000)).unwrap());
    }

    #[test]
    fn migrates_v1_database() {
        let dir = tempfile::tempdir().unwrap();
        {
            // 造一个只有贴图表的第 1 版数据库
            let conn = Connection::open(dir.path().join("history.db")).unwrap();
            conn.execute_batch(
                "CREATE TABLE pins (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at INTEGER NOT NULL, closed_at INTEGER NOT NULL,
                 width INTEGER NOT NULL, height INTEGER NOT NULL, text TEXT NOT NULL DEFAULT '', source TEXT NOT NULL,
                 bytes INTEGER NOT NULL DEFAULT 0); PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let h = History::open(dir.path()).unwrap();
        h.add_translation(tr("a", "甲", 1)).unwrap();
        assert_eq!(h.list_translations(None, 5).unwrap().len(), 1);
    }

    #[test]
    fn thumbnail_fits_and_keeps_ratio() {
        let t = thumbnail(&img(1280, 720)).unwrap();
        assert_eq!((t.width, t.height), (320, 180));
        let small = thumbnail(&img(100, 50)).unwrap();
        assert_eq!((small.width, small.height), (100, 50), "小图不放大");
    }

    #[test]
    fn add_writes_files_and_row() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        let r = add(&h, "hello 世界", 2_000);
        assert!(r.image_path.exists() && r.thumb_path.exists());
        assert_eq!(&fs::read(&r.image_path).unwrap()[1..4], b"PNG");
        assert_eq!(&fs::read(&r.thumb_path).unwrap()[..2], &[0xFF, 0xD8]);
        assert!(r.bytes > 0);
        let list = h.list_pins(None, 10).unwrap();
        assert_eq!(list, vec![r]);
    }

    #[test]
    fn list_newest_first_and_search() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        add(&h, "Alpha 一", 1_000);
        add(&h, "beta 二", 3_000);
        add(&h, "50% off_sale", 2_000);
        let ids: Vec<String> = h.list_pins(None, 10).unwrap().into_iter().map(|r| r.text).collect();
        assert_eq!(ids, vec!["beta 二", "50% off_sale", "Alpha 一"]);
        assert_eq!(h.list_pins(Some("ALPHA"), 10).unwrap().len(), 1, "不区分大小写");
        assert_eq!(h.list_pins(Some("二"), 10).unwrap().len(), 1);
        assert_eq!(h.list_pins(Some("%"), 10).unwrap().len(), 1, "% 当普通字符");
        assert_eq!(h.list_pins(Some("_"), 10).unwrap().len(), 1, "_ 当普通字符");
        assert_eq!(h.list_pins(Some("  "), 10).unwrap().len(), 3, "空白等于不搜索");
        assert_eq!(h.list_pins(None, 2).unwrap().len(), 2);
    }

    #[test]
    fn delete_and_usage() {
        let dir = tempfile::tempdir().unwrap();
        let h = History::open(dir.path()).unwrap();
        let a = add(&h, "a", 1_000);
        let b = add(&h, "b", 2_000);
        let u = h.pin_usage().unwrap();
        assert_eq!(u.count, 2);
        assert_eq!(u.bytes, a.bytes + b.bytes);
        assert!(h.delete_pin(a.id).unwrap());
        assert!(!a.image_path.exists());
        assert!(!h.delete_pin(a.id).unwrap());
        assert_eq!(h.pin_usage().unwrap(), Usage { count: 1, bytes: b.bytes });
    }

    #[test]
    fn reopen_keeps_data() {
        let dir = tempfile::tempdir().unwrap();
        let r = add(&History::open(dir.path()).unwrap(), "persist", 1_000);
        let h = History::open(dir.path()).unwrap();
        assert_eq!(h.list_pins(None, 10).unwrap(), vec![r]);
    }
}
