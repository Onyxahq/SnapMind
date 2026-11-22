//! 本地 OCR：ONNX Runtime（ort）+ PP-OCRv5 mobile 检测 / 识别模型。
//!
//! 流程：检测（DB 分割图 → 连通域 → 外扩成文字框）→ 每个框裁出来缩到 48 像素高 → 识别（CTC 贪心解码）。
//! 屏幕截图里的文字几乎都是水平的，所以文字框用轴对齐矩形，不做旋转矫正。
//!
//! 模型用 `node scripts/fetch-models.mjs` 下载到 `models/ppocrv5/`（det.onnx、rec.onnx、keys.txt）。

pub mod det;
pub mod eval;
mod imgops;
pub mod layout;
pub mod rec;

use std::path::Path;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use ort::session::builder::GraphOptimizationLevel;
use ort::session::Session;

pub use det::{DetOptions, TextBox};

#[derive(Debug, thiserror::Error)]
pub enum OcrError {
    #[error("找不到 OCR 模型文件：{0}")]
    MissingModel(String),
    #[error("ONNX Runtime 出错：{0}")]
    Ort(String),
    #[error("模型输出格式不对：{0}")]
    BadOutput(String),
    #[error("图片尺寸无效")]
    BadImage,
}

impl From<ort::Error> for OcrError {
    fn from(e: ort::Error) -> Self {
        OcrError::Ort(e.to_string())
    }
}

/// 一张图（RGBA，逐行紧密排列）。
#[derive(Debug, Clone, Copy)]
pub struct Image<'a> {
    pub width: u32,
    pub height: u32,
    pub rgba: &'a [u8],
}

/// 识别出的一行文字。坐标是输入图片的像素。
#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub text: String,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    /// 识别置信度（0–1，保留下来的字符的平均概率）
    pub score: f32,
}

/// 各阶段耗时，用来看性能。
#[derive(Debug, Clone, Copy, Default)]
pub struct Timing {
    pub det: Duration,
    pub rec: Duration,
    pub total: Duration,
}

pub struct Ocr {
    det: Mutex<Session>,
    /// 几个识别会话并行（ort 的会话一次只能跑一个推理）
    rec: Vec<Mutex<Session>>,
    keys: Vec<String>,
    pub det_options: DetOptions,
}

fn session(env: &ort::environment::Environment, path: &Path, threads: usize) -> Result<Session, OcrError> {
    if !path.exists() {
        return Err(OcrError::MissingModel(path.display().to_string()));
    }
    Ok(Session::builder(env)?
        .with_optimization_level(GraphOptimizationLevel::Level3)
        .map_err(|e| OcrError::Ort(e.to_string()))?
        .with_intra_threads(threads)
        .map_err(|e| OcrError::Ort(e.to_string()))?
        .commit_from_file(path)?)
}

impl Ocr {
    /// 从模型目录加载（det.onnx、rec.onnx、keys.txt）。加载一次后可以反复使用。
    pub fn load(dir: &Path) -> Result<Self, OcrError> {
        let keys_path = dir.join("keys.txt");
        let keys = std::fs::read_to_string(&keys_path).map_err(|_| OcrError::MissingModel(keys_path.display().to_string()))?;
        let keys: Vec<String> = keys.split('\n').map(|s| s.trim_end_matches('\r').to_string()).collect();
        let env = ort::init().with_name("snapmind-ocr").build()?;
        // 留几个核给界面；小机器至少 2 个线程
        let (det_threads, rec_sessions, rec_threads) = plan_threads();
        let det = session(&env, &dir.join("det.onnx"), det_threads)?;
        let rec = (0..rec_sessions)
            .map(|_| session(&env, &dir.join("rec.onnx"), rec_threads).map(Mutex::new))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { det: Mutex::new(det), rec, keys, det_options: DetOptions::default() })
    }

    /// 识别整张图，返回按阅读顺序排好的行。
    pub fn recognize(&self, img: Image) -> Result<(Vec<Line>, Timing), OcrError> {
        if img.width == 0 || img.height == 0 || img.rgba.len() < (img.width * img.height * 4) as usize {
            return Err(OcrError::BadImage);
        }
        let t0 = Instant::now();
        let boxes = det::detect(&mut self.det.lock().expect("OCR 检测锁"), img, &self.det_options)?;
        let t1 = Instant::now();
        let texts = rec::recognize_all(&self.rec, img, &boxes, &self.keys)?;
        let t2 = Instant::now();
        let mut lines: Vec<Line> = boxes
            .iter()
            .zip(texts)
            .filter(|(_, (text, _))| !text.trim().is_empty())
            .map(|(b, (text, score))| Line { text, x: b.x, y: b.y, width: b.width, height: b.height, score })
            .collect();
        sort_reading_order(&mut lines);
        Ok((lines, Timing { det: t1 - t0, rec: t2 - t1, total: t2 - t0 }))
    }
}

/// 线程分配：（检测线程数，识别会话数，每个识别会话的线程数）。留两个核给界面。
/// 环境变量 SNAPMIND_OCR_REC_SESSIONS / SNAPMIND_OCR_REC_THREADS 可以覆盖（调参用）。
fn plan_threads() -> (usize, usize, usize) {
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
    let usable = cores.saturating_sub(2).max(2);
    let env = |k: &str| std::env::var(k).ok().and_then(|s| s.parse::<usize>().ok()).filter(|&n| n > 0);
    // 实测（12 核）：2 个会话 × 5 线程最快；会话再多反而互相争抢
    let sessions = env("SNAPMIND_OCR_REC_SESSIONS").unwrap_or(if usable >= 4 { 2 } else { 1 });
    let threads = env("SNAPMIND_OCR_REC_THREADS").unwrap_or((usable / sessions).max(1));
    (usable.min(8), sessions, threads)
}

/// 阅读顺序：先按行（纵向重叠超过一半算同一行）从上到下，同一行内从左到右。
pub fn sort_reading_order(lines: &mut [Line]) {
    lines.sort_by_key(|l| (l.y + l.height / 2, l.x));
    // 插入排序式的修正：纵向中心落在前一行范围内、却在它左边的，换到前面
    for i in 1..lines.len() {
        let mut j = i;
        while j > 0 {
            let (a, b) = (&lines[j - 1], &lines[j]);
            let same_row = {
                let top = a.y.max(b.y);
                let bottom = (a.y + a.height).min(b.y + b.height);
                bottom > top && (bottom - top) * 2 > a.height.min(b.height)
            };
            if same_row && b.x < a.x {
                lines.swap(j - 1, j);
                j -= 1;
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn l(text: &str, x: u32, y: u32, w: u32, h: u32) -> Line {
        Line { text: text.into(), x, y, width: w, height: h, score: 1.0 }
    }

    #[test]
    fn reading_order_rows_then_columns() {
        // 同一行的两段：右边那段略高一点，也不能排到左边那段前面
        let mut v = vec![l("c", 0, 50, 100, 20), l("b", 300, 9, 100, 22), l("a", 0, 10, 100, 20)];
        sort_reading_order(&mut v);
        assert_eq!(v.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(), vec!["a", "b", "c"]);
    }
}
