//! 预处理：从原图的某个矩形里双线性采样，直接写成模型要的 CHW 浮点数（BGR 顺序，已归一化）。
//! 缩放和归一化一步完成，不产生中间图片。

use crate::Image;

/// 把 `src` 图里的矩形 `(sx, sy, sw, sh)` 缩放到 `out_w × out_h`，按 BGR 写入 `out`（CHW，宽度 `stride`），
/// 每个通道 `(v / 255 - mean) / std`。`stride` 大于 `out_w` 时右侧留给调用方填充。
#[allow(clippy::too_many_arguments)]
pub fn sample_chw(
    src: Image,
    (sx, sy, sw, sh): (f32, f32, f32, f32),
    out_w: usize,
    out_h: usize,
    stride: usize,
    mean: [f32; 3],
    std: [f32; 3],
    out: &mut [f32],
) {
    let plane = stride * out_h;
    let (w, h) = (src.width as usize, src.height as usize);
    let kx = sw / out_w as f32;
    let ky = sh / out_h as f32;
    // 预先算好每列的采样位置
    let cols: Vec<(usize, usize, f32)> = (0..out_w)
        .map(|x| {
            let fx = (sx + (x as f32 + 0.5) * kx - 0.5).clamp(0.0, (w - 1) as f32);
            let x0 = fx.floor() as usize;
            (x0, (x0 + 1).min(w - 1), fx - x0 as f32)
        })
        .collect();
    let inv = [1.0 / (255.0 * std[0]), 1.0 / (255.0 * std[1]), 1.0 / (255.0 * std[2])];
    let off = [mean[0] / std[0], mean[1] / std[1], mean[2] / std[2]];
    for y in 0..out_h {
        let fy = (sy + (y as f32 + 0.5) * ky - 0.5).clamp(0.0, (h - 1) as f32);
        let y0 = fy.floor() as usize;
        let y1 = (y0 + 1).min(h - 1);
        let ty = fy - y0 as f32;
        let r0 = &src.rgba[y0 * w * 4..(y0 + 1) * w * 4];
        let r1 = &src.rgba[y1 * w * 4..(y1 + 1) * w * 4];
        for (x, &(x0, x1, tx)) in cols.iter().enumerate() {
            let i = y * stride + x;
            // 输出通道 0/1/2 = B/G/R，对应 RGBA 里的 2/1/0
            for (c, src_c) in [2usize, 1, 0].into_iter().enumerate() {
                let a = r0[x0 * 4 + src_c] as f32;
                let b = r0[x1 * 4 + src_c] as f32;
                let cc = r1[x0 * 4 + src_c] as f32;
                let d = r1[x1 * 4 + src_c] as f32;
                let top = a + (b - a) * tx;
                let bottom = cc + (d - cc) * tx;
                let v = top + (bottom - top) * ty;
                out[c * plane + i] = v * inv[c] - off[c];
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_sampling_and_bgr_order() {
        // 2×1 图：红、蓝
        let px = [255u8, 0, 0, 255, 0, 0, 255, 255];
        let img = Image { width: 2, height: 1, rgba: &px };
        let mut out = vec![0f32; 3 * 2];
        sample_chw(img, (0.0, 0.0, 2.0, 1.0), 2, 1, 2, [0.0; 3], [1.0; 3], &mut out);
        // B 平面：0, 1；G：0, 0；R：1, 0
        assert_eq!(out, vec![0.0, 1.0, 0.0, 0.0, 1.0, 0.0]);
        // 归一化 (v/255 - 0.5) / 0.5：白 → 1，黑 → -1
        let px = [255u8, 255, 255, 255, 0, 0, 0, 255];
        let img = Image { width: 2, height: 1, rgba: &px };
        sample_chw(img, (0.0, 0.0, 2.0, 1.0), 2, 1, 2, [0.5; 3], [0.5; 3], &mut out);
        assert_eq!(&out[..2], &[1.0, -1.0]);
    }
}
