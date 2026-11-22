//! 文字识别：每个文字框缩到 48 像素高（宽度按比例）单独推理，CTC 贪心解码。
//!
//! 实测一次推理一行比按批推理快得多（批内要按最宽的一行填充），所以不分批；
//! 改成开几个识别会话并行处理，每个会话用少量线程（见 [`crate::Ocr::load`]）。

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;

use crate::det::TextBox;
use crate::imgops::sample_chw;
use crate::{Image, OcrError};

const HEIGHT: usize = 48;
/// 太短的行至少填充到这个宽度（与 PaddleOCR 推理时的最小宽度一致，短词识别更稳）
const MIN_WIDTH: usize = 320;
/// 太长的行截到这个宽度（3200 / 48 ≈ 66 倍高宽比，屏幕上的一行足够了）
const MAX_WIDTH: usize = 3200;

/// 识别所有文字框，返回与 `boxes` 一一对应的（文字，置信度）。
/// 多个会话并行：每个线程固定用一个会话，从共享计数器里领下一个框（长的先做，负载更均匀）。
pub fn recognize_all(
    sessions: &[Mutex<Session>],
    img: Image,
    boxes: &[TextBox],
    keys: &[String],
) -> Result<Vec<(String, f32)>, OcrError> {
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(width_for(&boxes[i])));
    let next = AtomicUsize::new(0);
    let results: Mutex<Vec<(String, f32)>> = Mutex::new(vec![(String::new(), 0.0); boxes.len()]);
    let first_err: Mutex<Option<OcrError>> = Mutex::new(None);
    let workers = sessions.len().min(boxes.len()).max(1);
    std::thread::scope(|s| {
        for session in sessions.iter().take(workers) {
            s.spawn(|| {
                let mut session = session.lock().expect("OCR 识别锁");
                loop {
                    let k = next.fetch_add(1, Ordering::Relaxed);
                    let Some(&i) = order.get(k) else { break };
                    match recognize_one(&mut session, img, &boxes[i], keys) {
                        Ok(r) => results.lock().expect("结果锁")[i] = r,
                        Err(e) => {
                            first_err.lock().expect("错误锁").get_or_insert(e);
                            break;
                        }
                    }
                }
            });
        }
    });
    if let Some(e) = first_err.into_inner().expect("错误锁") {
        return Err(e);
    }
    Ok(results.into_inner().expect("结果锁"))
}

/// 这个框缩到 48 像素高后的宽度。
fn width_for(b: &TextBox) -> usize {
    ((HEIGHT as f32 * b.width as f32 / b.height.max(1) as f32).ceil() as usize).clamp(8, MAX_WIDTH)
}

fn recognize_one(session: &mut Session, img: Image, b: &TextBox, keys: &[String]) -> Result<(String, f32), OcrError> {
    let w = width_for(b);
    let stride = w.max(MIN_WIDTH);
    // 填充区为 0（归一化后的灰色），和 PaddleOCR 一致
    let mut input = vec![0f32; 3 * HEIGHT * stride];
    sample_chw(img, (b.x as f32, b.y as f32, b.width as f32, b.height as f32), w, HEIGHT, stride, [0.5; 3], [0.5; 3], &mut input);
    let tensor = Tensor::from_array(([1usize, 3, HEIGHT, stride], input))?;
    let outputs = session.run(ort::inputs![tensor])?;
    let (shape, probs) = outputs[0].try_extract_tensor::<f32>()?;
    let dims: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    if dims.len() != 3 || dims[0] != 1 {
        return Err(OcrError::BadOutput(format!("识别输出形状 {dims:?}")));
    }
    let classes = dims[2];
    if classes != keys.len() + 2 {
        return Err(OcrError::BadOutput(format!("识别模型有 {classes} 类，字符表有 {} 个字符", keys.len())));
    }
    Ok(ctc_decode(probs, classes, keys))
}

/// CTC 贪心解码：每步取概率最大的类，去掉空白（0）和连续重复。
/// 类别 1..=N 对应字符表，N+1 是空格。置信度是保留下来的字符的平均概率。
pub fn ctc_decode(probs: &[f32], classes: usize, keys: &[String]) -> (String, f32) {
    let mut text = String::new();
    let (mut sum, mut n) = (0f32, 0usize);
    let mut prev = 0usize;
    for step in probs.chunks_exact(classes) {
        let (best, &p) = step.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).expect("类别数大于 0");
        if best != 0 && best != prev {
            match keys.get(best - 1) {
                Some(k) => text.push_str(k),
                None => text.push(' '),
            }
            sum += p;
            n += 1;
        }
        prev = best;
    }
    (text, if n == 0 { 0.0 } else { sum / n as f32 })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctc_removes_blanks_and_repeats() {
        let keys: Vec<String> = ["a", "b"].iter().map(|s| s.to_string()).collect();
        let classes = 4; // blank, a, b, space
        let step = |c: usize, p: f32| {
            let mut v = vec![(1.0 - p) / 3.0; classes];
            v[c] = p;
            v
        };
        // a a _ a b _ space b → "aab b"
        let seq: Vec<f32> = [(1, 0.9), (1, 0.9), (0, 0.9), (1, 0.8), (2, 0.7), (0, 0.9), (3, 0.6), (2, 0.9)]
            .iter()
            .flat_map(|&(c, p)| step(c, p))
            .collect();
        let (t, s) = ctc_decode(&seq, classes, &keys);
        assert_eq!(t, "aab b");
        assert!((s - (0.9 + 0.8 + 0.7 + 0.6 + 0.9) / 5.0).abs() < 1e-6);
        assert_eq!(ctc_decode(&step(0, 0.9), classes, &keys), (String::new(), 0.0));
    }

    #[test]
    fn widths() {
        assert_eq!(width_for(&TextBox { x: 0, y: 0, width: 400, height: 20, score: 1.0 }), 960);
        assert_eq!(width_for(&TextBox { x: 0, y: 0, width: 1, height: 100, score: 1.0 }), 8);
        assert_eq!(width_for(&TextBox { x: 0, y: 0, width: 9000, height: 10, score: 1.0 }), MAX_WIDTH);
    }
}
