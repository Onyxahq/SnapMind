//! 长截图拼接（需求 3.9、M9.1）。
//!
//! 选区固定，页面在下面滚动；每抓一帧就和上一帧比对，找出滚动了多少，把新露出来的部分接到长图后面。
//!
//! 做法（全部按「行」处理；横向长截图把每一列当成一行，算法完全一样）：
//! 1. 每行算一个指纹（64 位哈希）。行尾一小段不参与指纹——那里通常是滚动条，滚动时滑块在动；
//!    这一段里不跟着内容滚的列（滚动条的滑块、箭头）最后从长图边上裁掉；
//! 2. 上下两端和上一帧逐行相同的部分是固定的页头 / 页尾（导航栏、聊天输入框），不参与匹配；
//! 3. 中间部分投票：当前帧第 i 行和上一帧第 j 行指纹相同，就给「滚动了 j − i 行」投一票。
//!    纯色行、在上一帧里出现太多次的行（表格线、空行）不投票，免得把偏移投歪；
//! 4. 得票最多的偏移还要过两道关：投票行里至少一半认同它，并且按它对齐后整段重叠区里
//!    至少一半的行完全相同（行哈希 + 整段比对的双重校验）。过不了就算「对不上」，
//!    保留上一帧，等用户往回滚一点或放慢；
//! 5. 拼接时记住「长图的末尾对应上一帧的第几行」（end）。页尾那几行永远不拼进去，
//!    完成时再从最后一帧补上；页尾变高时把已经拼进去、现在其实属于页尾的几行退回去。
//!    所以长图里页头只出现一次（在最上面），页尾也只出现一次（在最下面）。

use crate::export::RgbaImage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 拼接方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Axis {
    /// 纵向：页面向下滚
    Vertical,
    /// 横向：页面向右滚
    Horizontal,
}

/// 推入一帧的结果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// 第一帧
    First,
    /// 没有滚动
    Same,
    /// 滚动了 `offset` 行，长图变长了 `added` 行（页尾变化时可能为 0 或负数）
    Moved { offset: usize, added: isize },
    /// 和上一帧对不上（滚得太快、内容变化太大、往回滚、或滚到了一片空白里）
    Lost,
    /// 已到长度上限，后面的内容不再拼接
    Full,
}

/// 页头 / 页尾最多占一帧的这个比例。
const BAND_MAX: f64 = 0.35;
/// 一个指纹在上一帧里出现超过这么多次就不参与投票（表格线、空行之类）。
const MAX_REPEAT: usize = 4;
/// 至少要有这么多行投票认同，才相信这个偏移。
const MIN_VOTES: u32 = 3;

/// 长图拼接器。帧是自上而下的 BGRA 像素（截屏的原始格式）。
pub struct Stitcher {
    axis: Axis,
    /// 帧的宽高（像素）
    width: u32,
    height: u32,
    /// 沿滚动方向有几行、每行几个像素
    rows: usize,
    span: usize,
    /// 每行末尾不参与指纹的像素数（滚动条）
    skip: usize,
    /// 拼好的行（BGRA，每行 span 个像素）
    out: Vec<u8>,
    /// 上一帧（行格式）和每行的指纹、是否有信息量
    prev: Vec<u8>,
    prev_hash: Vec<u64>,
    prev_info: Vec<bool>,
    /// 长图末尾对应上一帧的第几行（不含）
    end: usize,
    max_rows: usize,
    /// 行尾那一段（skip）里每一列：跟着内容一起滚的次数、原地不动或对不上的次数。
    /// 不跟着滚的列是滚动条（滑块、箭头），出图时从边上裁掉，免得在长图里重复出现
    movers: Vec<u32>,
    statics: Vec<u32>,
}

impl Stitcher {
    /// `max_rows`：长图沿滚动方向最多多少行。
    pub fn new(axis: Axis, width: u32, height: u32, max_rows: usize) -> Self {
        let (rows, span) = match axis {
            Axis::Vertical => (height as usize, width as usize),
            Axis::Horizontal => (width as usize, height as usize),
        };
        // 滚动条一般 8–20 物理像素宽，留 24；选区太窄时不扣
        let skip = if span > 200 { 24 } else { 0 };
        Self {
            axis,
            width,
            height,
            rows,
            span,
            skip,
            out: Vec::new(),
            prev: Vec::new(),
            prev_hash: Vec::new(),
            prev_info: Vec::new(),
            end: 0,
            max_rows: max_rows.max(rows),
            movers: vec![0; skip],
            statics: vec![0; skip],
        }
    }

    pub fn axis(&self) -> Axis {
        self.axis
    }

    fn stride(&self) -> usize {
        self.span * 4
    }

    /// 长图沿滚动方向的当前长度（含还没拼进去的页尾）。
    pub fn length(&self) -> usize {
        if self.prev.is_empty() {
            return 0;
        }
        self.out.len() / self.stride() + self.rows - self.end
    }

    /// 出图时从行尾裁掉几个像素：从最边上往里，直到最里面一列「不跟着滚」的列（滚动条）；
    /// 中间夹着纯色、看不出动没动的列（滚动条轨道的边）也一起裁，碰到跟着滚的内容列就停。
    fn crop(&self) -> usize {
        let mut crop = 0;
        for k in (0..self.skip).rev() {
            if self.movers[k] > self.statics[k] {
                break;
            }
            if self.statics[k] > self.movers[k] {
                crop = self.skip - k;
            }
        }
        crop
    }

    /// 长图当前的宽高（像素，已裁掉滚动条）。
    pub fn size(&self) -> (u32, u32) {
        let len = self.length() as u32;
        let span = (self.span - self.crop()) as u32;
        match self.axis {
            Axis::Vertical => (span, len),
            Axis::Horizontal => (len, span),
        }
    }

    /// 把一帧（BGRA，自上而下，宽高必须和创建时一致）转成行格式。
    fn to_rows(&self, bgra: &[u8]) -> Vec<u8> {
        match self.axis {
            Axis::Vertical => bgra.to_vec(),
            Axis::Horizontal => {
                let (w, h) = (self.width as usize, self.height as usize);
                let mut out = vec![0u8; w * h * 4];
                for y in 0..h {
                    for x in 0..w {
                        let s = (y * w + x) * 4;
                        let d = (x * h + y) * 4;
                        out[d..d + 4].copy_from_slice(&bgra[s..s + 4]);
                    }
                }
                out
            }
        }
    }

    /// 每行的指纹和「是否有信息量」（不是纯色）。
    fn fingerprint(&self, rows: &[u8]) -> (Vec<u64>, Vec<bool>) {
        let stride = self.stride();
        let used = (self.span - self.skip) * 4;
        let mut hash = Vec::with_capacity(self.rows);
        let mut info = Vec::with_capacity(self.rows);
        for r in 0..self.rows {
            let row = &rows[r * stride..r * stride + used];
            hash.push(row_hash(row));
            let first = &row[..4];
            // 至少 1% 的像素和第一个像素不同才算有信息量（只有一条细边框的行也算纯色）
            let need = (self.span / 100).max(1);
            let differ = row.chunks_exact(4).filter(|p| p[..3] != first[..3]).take(need).count();
            info.push(differ >= need);
        }
        (hash, info)
    }

    /// 推入一帧。
    pub fn push(&mut self, bgra: &[u8]) -> Step {
        let expect = self.width as usize * self.height as usize * 4;
        if bgra.len() < expect {
            return Step::Lost;
        }
        let cur = self.to_rows(&bgra[..expect]);
        let (hash, info) = self.fingerprint(&cur);
        if self.prev.is_empty() {
            self.out = cur.clone();
            self.end = self.rows;
            self.prev = cur;
            self.prev_hash = hash;
            self.prev_info = info;
            return Step::First;
        }
        if hash == self.prev_hash {
            self.prev = cur;
            return Step::Same;
        }

        let rows = self.rows;
        let band = (rows as f64 * BAND_MAX) as usize;
        let top = (0..rows).take_while(|&i| hash[i] == self.prev_hash[i]).count().min(band);
        let bot = (0..rows).rev().take_while(|&i| hash[i] == self.prev_hash[i]).count().min(band);
        let (lo, hi) = (top, rows - bot);
        if hi <= lo + 1 {
            self.prev = cur;
            return Step::Same;
        }

        // 上一帧中间部分的指纹 → 出现的位置
        let mut index: HashMap<u64, Vec<usize>> = HashMap::new();
        for j in lo..hi {
            if self.prev_info[j] {
                index.entry(self.prev_hash[j]).or_default().push(j);
            }
        }
        let eligible: Vec<bool> = (0..rows)
            .map(|i| info[i] && index.get(&hash[i]).map_or(true, |v| v.len() <= MAX_REPEAT))
            .collect();
        let mut votes = vec![0u32; rows];
        let mut still = 0u32;
        for i in lo..hi {
            if !eligible[i] {
                continue;
            }
            if let Some(list) = index.get(&hash[i]) {
                for &j in list {
                    if j > i {
                        votes[j - i] += 1;
                    } else if j == i {
                        still += 1;
                    }
                }
            }
        }
        let (offset, best) = votes.iter().enumerate().skip(1).fold((0, 0), |a, (d, &v)| if v > a.1 { (d, v) } else { a });
        if still >= best && still > 0 {
            // 中间部分大多原地没动：没有滚动，只是局部有变化（光标闪烁、悬停效果）
            self.prev = cur;
            self.prev_hash = hash;
            self.prev_info = info;
            return Step::Same;
        }
        if offset == 0 || best < MIN_VOTES {
            return Step::Lost;
        }
        // 第一关：本该参与投票的行里至少一半认同
        let voters = (lo..hi.saturating_sub(offset)).filter(|&i| eligible[i]).count() as u32;
        if best * 2 < voters {
            return Step::Lost;
        }
        // 第二关：按这个偏移对齐后，整段重叠区里至少一半的行完全相同
        let overlap = hi.saturating_sub(offset).saturating_sub(lo);
        let same = (lo..hi - offset).filter(|&i| hash[i] == self.prev_hash[i + offset]).count();
        if overlap == 0 || same * 2 < overlap {
            return Step::Lost;
        }

        // 行尾那一段的每一列：按这个偏移对齐后整列都对得上 = 跟着内容滚；对不上 = 滚动条。
        // 不对齐也整列对得上（纯色列）的看不出来，不投票
        let stride = self.stride();
        for k in 0..self.skip {
            let c = (self.span - self.skip + k) * 4;
            let px = |f: &[u8], r: usize| -> [u8; 3] {
                let i = r * stride + c;
                [f[i], f[i + 1], f[i + 2]]
            };
            let moves = (lo..hi - offset).all(|i| px(&cur, i) == px(&self.prev, i + offset));
            let still = (lo..hi - offset).all(|i| px(&cur, i) == px(&self.prev, i));
            if moves && !still {
                self.movers[k] += 1;
            } else if !moves {
                self.statics[k] += 1;
            }
        }
        // 页尾变高：已经拼进去、其实属于页尾的几行退回去（完成时从最后一帧补上）
        if offset > self.end {
            return Step::Lost; // 中间缺了一段，接不上
        }
        // 拼完以后的总长度正好是「现在的长度 + 偏移」（页尾跟着最后一帧走）
        if self.length() + offset > self.max_rows {
            return Step::Full;
        }
        let before = self.out.len() / stride;
        let target = rows - bot;
        if self.end > target {
            let back = (self.end - target).min(before);
            self.out.truncate(self.out.len() - back * stride);
            self.end -= back;
        }
        if offset > self.end {
            return Step::Lost;
        }
        let from = self.end - offset;
        if target > from {
            self.out.extend_from_slice(&cur[from * stride..target * stride]);
            self.end = target;
        } else {
            self.end = from;
        }
        self.prev = cur;
        self.prev_hash = hash;
        self.prev_info = info;
        Step::Moved { offset, added: (self.out.len() / stride) as isize - before as isize }
    }

    /// 长图的第 r 行（BGRA）。
    fn row(&self, r: usize) -> &[u8] {
        let stride = self.stride();
        let done = self.out.len() / stride;
        if r < done {
            &self.out[r * stride..(r + 1) * stride]
        } else {
            let p = self.end + (r - done);
            &self.prev[p * stride..(p + 1) * stride]
        }
    }

    /// 生成长图（RGBA）。`scale` < 1 时缩小（预览用，2×2 取平均）。
    pub fn image(&self, scale: f64) -> RgbaImage {
        let len = self.length();
        if len == 0 {
            return RgbaImage { width: 0, height: 0, data: Vec::new() };
        }
        let k = scale.clamp(0.01, 1.0);
        let span = self.span - self.crop();
        let (out_rows, out_span) = if k >= 1.0 {
            (len, span)
        } else {
            (((len as f64) * k).round().max(1.0) as usize, ((span as f64) * k).round().max(1.0) as usize)
        };
        // 先按行格式生成（RGBA），横向时最后转置
        let mut data = vec![0u8; out_rows * out_span * 4];
        for r in 0..out_rows {
            if k >= 1.0 {
                let src = self.row(r);
                for (d, s) in data[r * out_span * 4..(r + 1) * out_span * 4].chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d.copy_from_slice(&[s[2], s[1], s[0], 255]);
                }
                continue;
            }
            let r0 = ((r as f64 / k) as usize).min(len - 1);
            let r1 = (r0 + 1).min(len - 1);
            let (a, b) = (self.row(r0), self.row(r1));
            for c in 0..out_span {
                let c0 = ((c as f64 / k) as usize).min(span - 1);
                let c1 = (c0 + 1).min(span - 1);
                let px = |row: &[u8], x: usize, ch: usize| row[x * 4 + ch] as u32;
                let d = (r * out_span + c) * 4;
                for (ch, out_ch) in [(2, 0), (1, 1), (0, 2)] {
                    let s = px(a, c0, ch) + px(a, c1, ch) + px(b, c0, ch) + px(b, c1, ch);
                    data[d + out_ch] = ((s + 2) / 4) as u8;
                }
                data[d + 3] = 255;
            }
        }
        match self.axis {
            Axis::Vertical => RgbaImage { width: out_span as u32, height: out_rows as u32, data },
            Axis::Horizontal => {
                let mut t = vec![0u8; data.len()];
                for r in 0..out_rows {
                    for c in 0..out_span {
                        let s = (r * out_span + c) * 4;
                        let d = (c * out_rows + r) * 4;
                        t[d..d + 4].copy_from_slice(&data[s..s + 4]);
                    }
                }
                RgbaImage { width: out_rows as u32, height: out_span as u32, data: t }
            }
        }
    }
}

/// 一行像素的 64 位指纹。
fn row_hash(px: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut chunks = px.chunks_exact(8);
    for c in &mut chunks {
        let v = u64::from_le_bytes(c.try_into().expect("8 字节"));
        h = (h ^ v).wrapping_mul(0x0100_0000_01b3).rotate_left(29);
    }
    for &b in chunks.remainder() {
        h = (h ^ b as u64).wrapping_mul(0x0100_0000_01b3);
    }
    h ^ (px.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 确定性的伪随机数（测试用）。
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u32 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) as u32
        }
    }

    /// 一张「网页」：白底上一段段文字状的色块，中间有空白行和重复的分隔线。
    fn page(w: usize, h: usize, seed: u64) -> Vec<[u8; 4]> {
        let mut g = Lcg(seed);
        let mut px = vec![[255u8, 255, 255, 255]; w * h];
        let mut y = 0;
        while y < h {
            let kind = g.next() % 10;
            let lines = 4 + (g.next() % 14) as usize;
            if kind == 0 {
                // 分隔线：和别处的分隔线一模一样
                for x in 0..w {
                    px[y * w + x] = [200, 200, 200, 255];
                }
                y += 3;
                continue;
            }
            for dy in 0..lines.min(h - y) {
                if kind == 1 {
                    continue; // 空白
                }
                for x in 0..w {
                    if g.next() % 3 == 0 {
                        let v = (g.next() % 200) as u8;
                        px[(y + dy) * w + x] = [v, v / 2, 255 - v, 255];
                    }
                }
            }
            y += lines + 2;
        }
        px
    }

    /// 从页面上取一帧：页头 head 行、页尾 foot 行固定，中间是页面从 offset 开始的部分；
    /// 右侧 bar 列是滚动条（滑块随 offset 移动）。
    fn frame(pg: &[[u8; 4]], w: usize, h: usize, offset: usize, head: usize, foot: usize, bar: usize, total: usize) -> Vec<u8> {
        let mid = h - head - foot;
        let mut out = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                let p = if y < head {
                    [30, 60, (x % 256) as u8, 255]
                } else if y >= h - foot {
                    [90, (x % 256) as u8, 40, 255]
                } else {
                    pg[(offset + y - head) * w + x]
                };
                let p = if x >= w - bar {
                    let thumb = offset * h / total;
                    let edge = x >= w - 2; // 轨道最边上两列：纯色
                    let arrow = y + 30 >= h - foot && y < h - foot && x % 3 == 0; // 固定的箭头
                    if !edge && (arrow || (y >= thumb && y < thumb + 60)) { [120, 120, 120, 255] } else { [240, 240, 240, 255] }
                } else {
                    p
                };
                out.extend_from_slice(&p);
            }
        }
        let _ = mid;
        out
    }

    fn expected(pg: &[[u8; 4]], w: usize, h: usize, last: usize, head: usize, foot: usize) -> Vec<[u8; 4]> {
        let mid = h - head - foot;
        let mut rows = Vec::new();
        for _ in 0..head {
            for x in 0..w {
                rows.push([30, 60, (x % 256) as u8, 255]);
            }
        }
        for y in 0..last + mid {
            for x in 0..w {
                rows.push(pg[y * w + x]);
            }
        }
        for _ in 0..foot {
            for x in 0..w {
                rows.push([90, (x % 256) as u8, 40, 255]);
            }
        }
        rows
    }

    /// 比较拼好的图和期望（BGRA → RGBA）。滚动条那几列应该被裁掉。
    fn assert_matches(img: &RgbaImage, want: &[[u8; 4]], w: usize, bar: usize) {
        let out_w = w - bar;
        assert_eq!(img.width as usize, out_w, "滚动条没有裁掉");
        assert_eq!(img.height as usize, want.len() / w, "长度不对");
        for (i, got) in img.data.chunks_exact(4).enumerate() {
            let (x, y) = (i % out_w, i / out_w);
            let exp = want[y * w + x];
            assert_eq!(got, &[exp[2], exp[1], exp[0], 255], "像素 ({x}, {y}) 不对");
        }
    }

    #[test]
    fn stitches_with_fixed_header_footer_and_scrollbar() {
        let (w, h, head, foot, bar) = (320usize, 240usize, 30usize, 20usize, 12usize);
        let total = 2400;
        let pg = page(w, total, 7);
        let mut s = Stitcher::new(Axis::Vertical, w as u32, h as u32, 100_000);
        // 各种步长：没动、小步、大步、重复同一帧
        let offsets = [0usize, 0, 13, 40, 40, 41, 100, 150, 151, 260, 300, 420, 555, 700, 701, 850, 1000, 1150, 1300, 1450, 1600, 1750, 1900, 2050, 2210];
        let mut last = 0;
        for &o in &offsets {
            let step = s.push(&frame(&pg, w, h, o, head, foot, bar, total));
            assert!(!matches!(step, Step::Lost | Step::Full), "offset {o}: {step:?}");
            last = o;
        }
        assert_matches(&s.image(1.0), &expected(&pg, w, h, last, head, foot), w, bar);
    }

    #[test]
    fn stitches_without_fixed_parts() {
        let (w, h) = (300usize, 200usize);
        let total = 1500;
        let pg = page(w, total, 3);
        let mut s = Stitcher::new(Axis::Vertical, w as u32, h as u32, 100_000);
        let mut o = 0;
        while o + h < total {
            s.push(&frame(&pg, w, h, o, 0, 0, 0, total));
            o += 37;
        }
        let last = o - 37;
        assert_matches(&s.image(1.0), &expected(&pg, w, h, last, 0, 0), w, 0);
    }

    #[test]
    fn jump_too_far_is_lost_and_recovers() {
        let (w, h) = (300usize, 200usize);
        let pg = page(w, 1500, 11);
        let mut s = Stitcher::new(Axis::Vertical, w as u32, h as u32, 100_000);
        assert_eq!(s.push(&frame(&pg, w, h, 0, 0, 0, 0, 1500)), Step::First);
        // 一下滚过整整一帧多：对不上
        assert_eq!(s.push(&frame(&pg, w, h, 260, 0, 0, 0, 1500)), Step::Lost);
        assert_eq!(s.length(), h);
        // 往回滚到能对上的位置就接着拼
        assert!(matches!(s.push(&frame(&pg, w, h, 120, 0, 0, 0, 1500)), Step::Moved { offset: 120, .. }));
        assert_eq!(s.length(), h + 120);
    }

    #[test]
    fn scrolling_back_does_not_append() {
        let (w, h) = (300usize, 200usize);
        let pg = page(w, 1500, 5);
        let mut s = Stitcher::new(Axis::Vertical, w as u32, h as u32, 100_000);
        s.push(&frame(&pg, w, h, 100, 0, 0, 0, 1500));
        assert_eq!(s.push(&frame(&pg, w, h, 60, 0, 0, 0, 1500)), Step::Lost);
        assert_eq!(s.length(), h);
    }

    #[test]
    fn blank_area_is_not_matched_by_guessing() {
        let (w, h) = (300usize, 200usize);
        let white = vec![255u8; w * h * 4];
        let mut grey = white.clone();
        for p in grey.chunks_exact_mut(4).take(w * 10) {
            p[0] = 0;
        }
        let mut s = Stitcher::new(Axis::Vertical, w as u32, h as u32, 100_000);
        s.push(&white);
        assert_eq!(s.push(&white), Step::Same);
        // 只有一条纯色带变化：没有可投票的行，不瞎猜
        assert_eq!(s.push(&grey), Step::Lost);
    }

    #[test]
    fn stops_at_max_length() {
        let (w, h) = (300usize, 200usize);
        let pg = page(w, 1500, 9);
        let mut s = Stitcher::new(Axis::Vertical, w as u32, h as u32, 300);
        s.push(&frame(&pg, w, h, 0, 0, 0, 0, 1500));
        assert!(matches!(s.push(&frame(&pg, w, h, 80, 0, 0, 0, 1500)), Step::Moved { .. }));
        assert_eq!(s.push(&frame(&pg, w, h, 160, 0, 0, 0, 1500)), Step::Full);
        assert_eq!(s.length(), 280);
    }

    #[test]
    fn horizontal_is_transposed_vertical() {
        // 把一张纵向的页面转置成横向页面，横向拼接的结果再转置回来应该一样
        let (w, h) = (200usize, 300usize); // 帧：宽 200、高 300；横向滚动
        let total = 1200;
        let pg = page(h, total, 21); // 每「行」h 个像素
        let mut s = Stitcher::new(Axis::Horizontal, w as u32, h as u32, 100_000);
        let mut last = 0;
        for o in (0..=900).step_by(45) {
            // 横向帧：第 x 列 = 页面第 o + x 行
            let mut f = vec![0u8; w * h * 4];
            for y in 0..h {
                for x in 0..w {
                    let p = pg[(o + x) * h + y];
                    f[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&p);
                }
            }
            assert!(!matches!(s.push(&f), Step::Lost), "offset {o}");
            last = o;
        }
        let img = s.image(1.0);
        assert_eq!((img.width as usize, img.height as usize), (last + w, h));
        for x in [0usize, 333, last + w - 1] {
            for y in [0usize, 150, h - 1] {
                let i = (y * img.width as usize + x) * 4;
                let p = pg[x * h + y];
                assert_eq!(&img.data[i..i + 4], &[p[2], p[1], p[0], 255], "({x}, {y})");
            }
        }
    }

    #[test]
    fn preview_is_scaled() {
        let (w, h) = (300usize, 200usize);
        let pg = page(w, 1500, 13);
        let mut s = Stitcher::new(Axis::Vertical, w as u32, h as u32, 100_000);
        s.push(&frame(&pg, w, h, 0, 0, 0, 0, 1500));
        s.push(&frame(&pg, w, h, 100, 0, 0, 0, 1500));
        let p = s.image(0.5);
        assert_eq!((p.width, p.height), (150, 150));
        assert_eq!(p.data.len(), 150 * 150 * 4);
    }
}
