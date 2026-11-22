//! 滚动日志文件（M11.2）。
//!
//! 一个文件写满 `max` 字节就改名为 `snapmind.1.log`（原来的 1 变 2……），最多留 `keep` 个旧文件。
//! 每行：`2026-11-23 10:00:00.123 [warn] 内容`，多行内容的后续行缩进两格，方便一眼看出一条记录的边界。

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use chrono::NaiveDateTime;

/// 当前日志文件名。
pub const FILE: &str = "snapmind.log";
/// 单个文件的上限：1 MB 足够装下几千条记录，发给别人也不大。
pub const MAX_BYTES: u64 = 1024 * 1024;
/// 保留的旧文件个数。
pub const KEEP: usize = 2;

pub struct Log {
    dir: PathBuf,
    file: File,
    size: u64,
    max: u64,
    keep: usize,
}

/// 第 `n` 个旧文件（0 为当前文件）。
fn rolled(dir: &Path, n: usize) -> PathBuf {
    if n == 0 {
        dir.join(FILE)
    } else {
        dir.join(format!("snapmind.{n}.log"))
    }
}

/// 一条记录排成的文本（带换行）。
pub fn line(level: &str, msg: &str, now: NaiveDateTime) -> String {
    let mut s = format!("{} [{level}] ", now.format("%Y-%m-%d %H:%M:%S%.3f"));
    for (i, l) in msg.trim_end().lines().enumerate() {
        if i > 0 {
            s.push_str("\n  ");
        }
        s.push_str(l);
    }
    s.push('\n');
    s
}

impl Log {
    pub fn open(dir: &Path, max: u64, keep: usize) -> io::Result<Self> {
        fs::create_dir_all(dir)?;
        let file = OpenOptions::new().create(true).append(true).open(rolled(dir, 0))?;
        let size = file.metadata()?.len();
        Ok(Self { dir: dir.to_path_buf(), file, size, max, keep })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn write(&mut self, level: &str, msg: &str, now: NaiveDateTime) -> io::Result<()> {
        let text = line(level, msg, now);
        if self.size > 0 && self.size + text.len() as u64 > self.max {
            self.roll()?;
        }
        self.file.write_all(text.as_bytes())?;
        self.size += text.len() as u64;
        Ok(())
    }

    fn roll(&mut self) -> io::Result<()> {
        let _ = fs::remove_file(rolled(&self.dir, self.keep));
        for n in (0..self.keep).rev() {
            let from = rolled(&self.dir, n);
            if from.exists() {
                fs::rename(&from, rolled(&self.dir, n + 1))?;
            }
        }
        if self.keep == 0 {
            let _ = fs::remove_file(rolled(&self.dir, 0));
        }
        self.file = OpenOptions::new().create(true).append(true).open(rolled(&self.dir, 0))?;
        self.size = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t() -> NaiveDateTime {
        NaiveDateTime::parse_from_str("2026-11-23 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap()
    }

    #[test]
    fn line_format_indents_continuations() {
        assert_eq!(line("warn", "a\nb\n", t()), "2026-11-23 10:00:00.000 [warn] a\n  b\n");
    }

    #[test]
    fn appends_across_opens() {
        let dir = tempfile::tempdir().unwrap();
        Log::open(dir.path(), 1000, 2).unwrap().write("info", "one", t()).unwrap();
        Log::open(dir.path(), 1000, 2).unwrap().write("info", "two", t()).unwrap();
        let s = fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(s.contains("one") && s.contains("two"));
    }

    #[test]
    fn rolls_and_keeps_limited_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = Log::open(dir.path(), 100, 2).unwrap();
        for i in 0..10 {
            log.write("info", &format!("entry {i} {}", "x".repeat(40)), t()).unwrap();
        }
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        assert_eq!(names.len(), 3, "{names:?}");
        // 当前文件只有最后一条，最旧的那些已经丢掉
        let cur = fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(cur.contains("entry 9") && !cur.contains("entry 8"));
        assert!(fs::read_to_string(rolled(dir.path(), 1)).unwrap().contains("entry 8"));
        assert!(fs::read_to_string(rolled(dir.path(), 2)).unwrap().contains("entry 7"));
        for n in [1, 2] {
            assert!(fs::metadata(rolled(dir.path(), n)).unwrap().len() <= 100);
        }
    }

    #[test]
    fn oversized_entry_still_written() {
        let dir = tempfile::tempdir().unwrap();
        let mut log = Log::open(dir.path(), 10, 1).unwrap();
        log.write("error", &"y".repeat(50), t()).unwrap();
        assert!(fs::read_to_string(dir.path().join(FILE)).unwrap().contains(&"y".repeat(50)));
    }
}
