//! 崩溃恢复（M11.2）：开着的贴图在磁盘上留一份，正常关闭或退出时删掉。
//!
//! 下次启动时目录里还有文件，说明上次没有正常退出（崩溃、被结束进程、关机时没来得及退出），
//! 按记下的位置和大小把这些贴图重新贴出来。每张贴图两个文件：`<id>.json` 描述，`<id>.png` 图片
//! （文字卡片没有图片，按原文重新排版）。

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::export::{self, ImageFormat, RgbaImage};
use crate::history::PinSource;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinMeta {
    pub source: PinSource,
    /// 文字卡片的原文
    pub text: Option<String>,
    pub created_at: i64,
    /// 窗口外框（物理像素）：x、y、宽、高
    pub rect: (i32, i32, u32, u32),
    /// 所在显示器的缩放
    pub scale: f64,
}

/// 上次留下的一张贴图。
pub struct Leftover {
    pub meta: PinMeta,
    pub image: Option<RgbaImage>,
}

pub struct Session {
    dir: PathBuf,
}

/// 先写临时文件再改名，半截文件不会被当成完整的读进来。
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)
}

fn other<E: std::fmt::Display>(e: E) -> io::Error {
    io::Error::other(e.to_string())
}

impl Session {
    pub fn new(dir: &Path) -> Self {
        Self { dir: dir.to_path_buf() }
    }

    fn json(&self, id: u32) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    fn png(&self, id: u32) -> PathBuf {
        self.dir.join(format!("{id}.png"))
    }

    pub fn save_meta(&self, id: u32, meta: &PinMeta) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        write_atomic(&self.json(id), &serde_json::to_vec(meta).map_err(other)?)
    }

    pub fn save_image(&self, id: u32, image: &RgbaImage) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        // 快速压缩：贴图常开常关，写这一份不该拖慢别的事
        let bytes = export::encode(image, ImageFormat::Png, 100, true).map_err(other)?;
        write_atomic(&self.png(id), &bytes)
    }

    pub fn remove(&self, id: u32) {
        let _ = fs::remove_file(self.json(id));
        let _ = fs::remove_file(self.png(id));
    }

    pub fn clear(&self) {
        let _ = fs::remove_dir_all(&self.dir);
    }

    /// 取出上次留下的全部贴图（按 id 排序），并清空目录。读不了的跳过。
    pub fn take_all(&self) -> Vec<Leftover> {
        let Ok(rd) = fs::read_dir(&self.dir) else { return Vec::new() };
        let mut ids: Vec<u32> = rd
            .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".json")?.parse().ok())
            .collect();
        ids.sort_unstable();
        let out = ids
            .into_iter()
            .filter_map(|id| {
                let meta: PinMeta = serde_json::from_slice(&fs::read(self.json(id)).ok()?).ok()?;
                let image = fs::read(self.png(id)).ok().and_then(|b| export::decode_png(&b).ok());
                // 截图贴图没有图片就没法恢复；文字卡片可以按原文重排
                if image.is_none() && meta.text.is_none() {
                    return None;
                }
                Some(Leftover { meta, image })
            })
            .collect();
        self.clear();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(text: Option<&str>) -> PinMeta {
        PinMeta {
            source: if text.is_some() { PinSource::Text } else { PinSource::Capture },
            text: text.map(String::from),
            created_at: 42,
            rect: (10, 20, 300, 200),
            scale: 1.5,
        }
    }

    fn img() -> RgbaImage {
        RgbaImage { width: 2, height: 1, data: vec![255, 0, 0, 255, 0, 255, 0, 128] }
    }

    #[test]
    fn round_trip_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let s = Session::new(&dir.path().join("session"));
        s.save_meta(3, &meta(None)).unwrap();
        s.save_image(3, &img()).unwrap();
        s.save_meta(1, &meta(Some("你好"))).unwrap();
        // 拖动后整份描述重写
        s.save_meta(3, &PinMeta { rect: (5, 6, 7, 8), scale: 2.0, ..meta(None) }).unwrap();
        let got = s.take_all();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].meta.text.as_deref(), Some("你好"));
        assert!(got[0].image.is_none());
        assert_eq!(got[1].meta.rect, (5, 6, 7, 8));
        assert_eq!(got[1].meta.scale, 2.0);
        assert_eq!(got[1].image.as_ref().unwrap().data, img().data);
        // 取出后目录清空，再取就没有了
        assert!(s.take_all().is_empty());
    }

    #[test]
    fn removed_and_broken_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let s = Session::new(dir.path());
        s.save_meta(1, &meta(None)).unwrap();
        s.save_image(1, &img()).unwrap();
        s.remove(1);
        // 截图贴图缺图片：没法恢复
        s.save_meta(2, &meta(None)).unwrap();
        // 描述文件坏了
        fs::write(dir.path().join("4.json"), b"{oops").unwrap();
        assert!(s.take_all().is_empty());
    }
}
