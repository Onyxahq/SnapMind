//! 文字检测：PP-OCRv5 DB 模型输出每个像素是文字的概率，二值化后取连通域，按 DB 的规则外扩成文字框。

use ort::session::Session;
use ort::value::Tensor;

use crate::imgops::sample_chw;
use crate::{Image, OcrError};

#[derive(Debug, Clone, Copy)]
pub struct DetOptions {
    /// 送进模型的长边上限（像素，会取整到 32 的倍数）。实测 12 px 的屏幕文字缩到 1440 仍然零错误，
    /// 检测耗时比 1920 少一半；缩到 960 开始漏字。
    pub max_side: u32,
    /// 概率二值化阈值
    pub thresh: f32,
    /// 文字框内平均概率低于它就丢掉
    pub box_thresh: f32,
    /// 外扩比例（DB 的 unclip_ratio）
    pub unclip_ratio: f32,
    /// 短边小于这个像素数（模型输入尺度上）的框丢掉
    pub min_size: u32,
}

impl Default for DetOptions {
    fn default() -> Self {
        Self { max_side: 1440, thresh: 0.3, box_thresh: 0.6, unclip_ratio: 1.5, min_size: 3 }
    }
}

/// 检测出的文字框（原图像素）。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub score: f32,
}

const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// 模型输入尺寸：长边不超过 max_side，宽高都取整到 32 的倍数（至少 32）。小图不放大。
pub fn input_size(w: u32, h: u32, max_side: u32) -> (u32, u32) {
    let k = (max_side as f32 / w.max(h) as f32).min(1.0);
    let r = |v: u32| (((v as f32 * k) / 32.0).round() as u32).max(1) * 32;
    (r(w), r(h))
}

pub fn detect(session: &mut Session, img: Image, opt: &DetOptions) -> Result<Vec<TextBox>, OcrError> {
    let (iw, ih) = input_size(img.width, img.height, opt.max_side);
    let (iw_, ih_) = (iw as usize, ih as usize);
    let mut input = vec![0f32; 3 * iw_ * ih_];
    sample_chw(img, (0.0, 0.0, img.width as f32, img.height as f32), iw_, ih_, iw_, MEAN, STD, &mut input);
    let tensor = Tensor::from_array(([1usize, 3, ih_, iw_], input))?;
    let outputs = session.run(ort::inputs![tensor])?;
    let (shape, prob) = outputs[0].try_extract_tensor::<f32>()?;
    let dims: Vec<i64> = shape.iter().copied().collect();
    if dims.len() != 4 || dims[2] as usize != ih_ || dims[3] as usize != iw_ {
        return Err(OcrError::BadOutput(format!("检测输出形状 {dims:?}")));
    }
    let boxes = boxes_from_prob(prob, iw_, ih_, opt);
    // 换回原图坐标
    let (kx, ky) = (img.width as f32 / iw as f32, img.height as f32 / ih as f32);
    Ok(boxes
        .into_iter()
        .filter_map(|(x0, y0, x1, y1, score)| {
            let bx0 = (x0 * kx).floor().max(0.0) as u32;
            let by0 = (y0 * ky).floor().max(0.0) as u32;
            let bx1 = ((x1 * kx).ceil() as u32).min(img.width);
            let by1 = ((y1 * ky).ceil() as u32).min(img.height);
            (bx1 > bx0 && by1 > by0).then_some(TextBox { x: bx0, y: by0, width: bx1 - bx0, height: by1 - by0, score })
        })
        .collect())
}

/// DB 后处理：二值化 → 8 连通域 → 外接矩形 → 按平均概率过滤 → 按面积 / 周长外扩。
/// 返回模型输入尺度上的 (x0, y0, x1, y1, score)。
pub fn boxes_from_prob(prob: &[f32], w: usize, h: usize, opt: &DetOptions) -> Vec<(f32, f32, f32, f32, f32)> {
    let mut label = vec![0u32; w * h];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    let mut next = 0u32;
    for start in 0..w * h {
        if label[start] != 0 || prob[start] <= opt.thresh {
            continue;
        }
        next += 1;
        label[start] = next;
        stack.push(start);
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        let (mut sum, mut n) = (0f64, 0usize);
        while let Some(p) = stack.pop() {
            let (x, y) = (p % w, p / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
            sum += prob[p] as f64;
            n += 1;
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if (dx, dy) == (0, 0) || nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let q = ny as usize * w + nx as usize;
                    if label[q] == 0 && prob[q] > opt.thresh {
                        label[q] = next;
                        stack.push(q);
                    }
                }
            }
        }
        let (bw, bh) = ((x1 - x0 + 1) as f32, (y1 - y0 + 1) as f32);
        if bw.min(bh) < opt.min_size as f32 {
            continue;
        }
        let score = (sum / n as f64) as f32;
        if score < opt.box_thresh {
            continue;
        }
        // DB 的外扩：距离 = 面积 × 比例 / 周长
        let d = bw * bh * opt.unclip_ratio / (2.0 * (bw + bh));
        out.push((
            (x0 as f32 - d).max(0.0),
            (y0 as f32 - d).max(0.0),
            (x1 as f32 + 1.0 + d).min(w as f32),
            (y1 as f32 + 1.0 + d).min(h as f32),
            score,
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_size_rounds_to_32_and_never_upscales() {
        assert_eq!(input_size(1920, 1080, 1920), (1920, 1088));
        assert_eq!(input_size(3840, 2160, 1920), (1920, 1088));
        assert_eq!(input_size(100, 20, 1920), (96, 32));
        assert_eq!(input_size(10, 10, 1920), (32, 32));
    }

    #[test]
    fn two_blobs_become_two_expanded_boxes() {
        let (w, h) = (40, 20);
        let mut p = vec![0f32; w * h];
        // 第一块：x 2..12, y 4..8（概率 0.9）；第二块：x 20..36, y 10..16
        for y in 4..8 {
            for x in 2..12 {
                p[y * w + x] = 0.9;
            }
        }
        for y in 10..16 {
            for x in 20..36 {
                p[y * w + x] = 0.8;
            }
        }
        // 一块概率不够的噪点（低于 box_thresh）
        p[0] = 0.5;
        p[1] = 0.5;
        p[w] = 0.5;
        p[w + 1] = 0.5;
        p[2 * w] = 0.5;
        p[2 * w + 1] = 0.5;
        let b = boxes_from_prob(&p, w, h, &DetOptions::default());
        assert_eq!(b.len(), 2);
        let (x0, y0, x1, y1, s) = b[0];
        // 10×4 的块：d = 40×1.5/28 ≈ 2.14
        // 左边外扩后越界，夹到 0
        assert!(x0 == 0.0 && (y1 - (8.0 + 2.142857)).abs() < 1e-3, "{:?}", b[0]);
        assert!(x1 > 12.0 && y0 < 4.0 && (s - 0.9).abs() < 1e-6);
        assert!((b[1].4 - 0.8).abs() < 1e-6);
    }
}
