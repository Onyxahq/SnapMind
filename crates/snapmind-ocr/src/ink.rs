//! 从原图量出一行文字笔画的实际范围和颜色。
//!
//! 检测框是按 DB 规则外扩过的，高度随行长、模型而变，不能代表字号；段落规则要的「字号接近」「行间距」
//! 用笔画的真实上下边来算。文字颜色、背景颜色用来区分标题和说明、不同的气泡或卡片。

use crate::Image;

/// 一行文字的笔画范围（原图像素）和颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Ink {
    pub left: u32,
    pub top: u32,
    pub width: u32,
    pub height: u32,
    /// 文字颜色（RGB，取最「浓」的那部分笔画，避开抗锯齿的半透明边）
    pub color: [u8; 3],
    /// 背景颜色（框四边像素的中位数）
    pub background: [u8; 3],
}

impl Ink {
    pub fn right(&self) -> u32 {
        self.left + self.width
    }
    pub fn bottom(&self) -> u32 {
        self.top + self.height
    }
}

/// 两种颜色的距离（各通道差的最大值，0–255）。
pub fn color_distance(a: [u8; 3], b: [u8; 3]) -> u8 {
    (0..3).map(|i| a[i].abs_diff(b[i])).max().unwrap_or(0)
}

fn median(v: &mut [u8]) -> u8 {
    if v.is_empty() {
        return 0;
    }
    let mid = v.len() / 2;
    *v.select_nth_unstable(mid).1
}

/// 量出框 `(x, y, w, h)` 里的笔画。框里没有明显的笔画时退回整个框。
pub fn measure(img: Image, x: u32, y: u32, w: u32, h: u32) -> Ink {
    let x1 = (x + w).min(img.width);
    let y1 = (y + h).min(img.height);
    let whole = |bg: [u8; 3]| Ink {
        left: x,
        top: y,
        width: x1 - x,
        height: y1 - y,
        color: bg,
        background: bg,
    };
    if x1 <= x + 2 || y1 <= y + 2 {
        return whole([0; 3]);
    }
    let stride = img.width as usize * 4;
    let px = |xx: u32, yy: u32| {
        let i = yy as usize * stride + xx as usize * 4;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
    };

    // 背景：四条边上像素各通道的中位数（框外扩过，边上基本是背景；紧挨着的别的行的笔画是少数）
    let mut border: [Vec<u8>; 3] = Default::default();
    let mut push = |c: [u8; 3]| (0..3).for_each(|i| border[i].push(c[i]));
    for xx in x..x1 {
        push(px(xx, y));
        push(px(xx, y1 - 1));
    }
    for yy in y + 1..y1 - 1 {
        push(px(x, yy));
        push(px(x1 - 1, yy));
    }
    let bg = [
        median(&mut border[0]),
        median(&mut border[1]),
        median(&mut border[2]),
    ];

    // 每个像素和背景的差；阈值随这一行最浓的笔画自适应（低对比的浅灰字也能量出来）
    let (bw, bh) = ((x1 - x) as usize, (y1 - y) as usize);
    let mut diff = vec![0u8; bw * bh];
    let mut hist = [0usize; 256];
    for (r, yy) in (y..y1).enumerate() {
        for (c, xx) in (x..x1).enumerate() {
            let d = color_distance(px(xx, yy), bg);
            diff[r * bw + c] = d;
            hist[d as usize] += 1;
        }
    }
    // 第 99 百分位当作「最浓的笔画」
    let mut acc = 0;
    let target = diff.len() - diff.len() / 100;
    let peak = (0..256).find(|&v| {
        acc += hist[v];
        acc >= target
    });
    let peak = peak.unwrap_or(255) as u8;
    if peak < 20 {
        return whole(bg);
    }
    let t = ((peak as f32 * 0.4) as u8).max(12);

    // 每行有没有笔画；把间隔不超过 gap 行的连成一段（i、j 的点和字身之间有空隙），取笔画最多的那段，
    // 挤进框里的上下相邻行的笔画就被排除了
    let rows: Vec<usize> = (0..bh)
        .map(|r| {
            diff[r * bw..(r + 1) * bw]
                .iter()
                .filter(|&&d| d >= t)
                .count()
        })
        .collect();
    let gap = (bh / 10).max(1);
    let (mut best, mut best_sum) = ((0, 0), 0usize);
    let mut r = 0;
    while r < bh {
        if rows[r] == 0 {
            r += 1;
            continue;
        }
        let (start, mut end, mut sum, mut blank) = (r, r, 0usize, 0usize);
        while r < bh && blank <= gap {
            if rows[r] > 0 {
                end = r;
                sum += rows[r];
                blank = 0;
            } else {
                blank += 1;
            }
            r += 1;
        }
        if sum > best_sum {
            best = (start, end);
            best_sum = sum;
        }
    }
    if best_sum == 0 {
        return whole(bg);
    }
    let (r0, r1) = best;

    // 左右范围和颜色只看这一段
    let strong = t.max((peak as f32 * 0.7) as u8);
    let (mut c0, mut c1) = (bw, 0);
    let (mut sum, mut n) = ([0u64; 3], 0u64);
    for r in r0..=r1 {
        for c in 0..bw {
            let d = diff[r * bw + c];
            if d >= t {
                c0 = c0.min(c);
                c1 = c1.max(c);
            }
            if d >= strong {
                let p = px(x + c as u32, y + r as u32);
                (0..3).for_each(|i| sum[i] += p[i] as u64);
                n += 1;
            }
        }
    }
    let n = n.max(1);
    Ink {
        left: x + c0 as u32,
        top: y + r0 as u32,
        width: (c1 + 1 - c0) as u32,
        height: (r1 + 1 - r0) as u32,
        color: [(sum[0] / n) as u8, (sum[1] / n) as u8, (sum[2] / n) as u8],
        background: bg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 白底上画一块深色「笔画」，上方挤进来一条别的行的笔画
    #[test]
    fn measures_main_band_and_colors() {
        let (w, h) = (40u32, 30u32);
        let mut rgba = vec![255u8; (w * h * 4) as usize];
        let mut fill = |x0: u32, y0: u32, x1: u32, y1: u32, c: [u8; 3]| {
            for y in y0..y1 {
                for x in x0..x1 {
                    let i = ((y * w + x) * 4) as usize;
                    rgba[i..i + 3].copy_from_slice(&c);
                }
            }
        };
        fill(5, 1, 30, 3, [0, 0, 0]); // 上一行露进来的笔画
        fill(8, 10, 32, 22, [40, 40, 200]); // 这一行
        let ink = measure(
            Image {
                width: w,
                height: h,
                rgba: &rgba,
            },
            0,
            0,
            w,
            h,
        );
        assert_eq!((ink.left, ink.top, ink.width, ink.height), (8, 10, 24, 12));
        assert_eq!(ink.background, [255, 255, 255]);
        assert_eq!(ink.color, [40, 40, 200]);
        // 空白框退回整个框
        let blank = vec![255u8; (w * h * 4) as usize];
        let ink = measure(
            Image {
                width: w,
                height: h,
                rgba: &blank,
            },
            2,
            3,
            10,
            10,
        );
        assert_eq!((ink.left, ink.top, ink.width, ink.height), (2, 3, 10, 10));
    }
}
