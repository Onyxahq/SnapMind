//! 录屏存成 GIF（M9.2）。
//!
//! - 每帧单独取 256 色调色板（NeuQuant），屏幕内容颜色少，效果够用；
//! - 只编码和上一帧不同的那块矩形（其余保留上一帧），屏幕录像大部分时间只有一小块在变，文件小很多；
//! - 和上一帧完全一样的帧不写，延时累加到上一帧上；延时按真实时间戳算（10 ms 为单位，
//!   按累计时间取整，长时间录制也不漂移）。
//!
//! 没用 gifski：它是 AGPL 许可，而且画质优势在照片类内容上，屏幕录像用不上。

use std::io::Write;
use std::time::Duration;

#[derive(Debug, thiserror::Error)]
pub enum GifError {
    #[error("GIF 编码失败：{0}")]
    Encode(#[from] gif::EncodingError),
    #[error("写 GIF 失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("帧大小不对")]
    BadFrame,
}

struct Pending {
    frame: gif::Frame<'static>,
    /// 这一帧开始的时间（10 ms 单位，按累计时间取整）
    at: u32,
}

pub struct GifWriter<W: Write> {
    enc: gif::Encoder<W>,
    width: u16,
    height: u16,
    /// 已经显示出来的画面（RGBA），用来和新帧比较
    canvas: Vec<u8>,
    pending: Option<Pending>,
    /// NeuQuant 的速度（1 最慢最好，30 最快）
    speed: i32,
}

/// 两帧（RGBA）不同的像素所在的最小矩形；完全一样返回 None。
pub fn diff_box(a: &[u8], b: &[u8], width: usize, height: usize) -> Option<(usize, usize, usize, usize)> {
    let row = width * 4;
    let top = (0..height).find(|&y| a[y * row..(y + 1) * row] != b[y * row..(y + 1) * row])?;
    let bottom = (0..height).rev().find(|&y| a[y * row..(y + 1) * row] != b[y * row..(y + 1) * row])?;
    let mut left = width;
    let mut right = 0;
    for y in top..=bottom {
        let (ra, rb) = (&a[y * row..(y + 1) * row], &b[y * row..(y + 1) * row]);
        if let Some(l) = (0..left).find(|&x| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4]) {
            left = l;
        }
        if let Some(r) = (right..width).rev().find(|&x| ra[x * 4..x * 4 + 4] != rb[x * 4..x * 4 + 4]) {
            right = right.max(r);
        }
    }
    Some((left, top, right - left + 1, bottom - top + 1))
}

fn centis(t: Duration) -> u32 {
    (t.as_millis() / 10) as u32
}

impl<W: Write> GifWriter<W> {
    pub fn new(out: W, width: u32, height: u32) -> Result<Self, GifError> {
        if width == 0 || height == 0 || width > u16::MAX as u32 || height > u16::MAX as u32 {
            return Err(GifError::BadFrame);
        }
        let (w, h) = (width as u16, height as u16);
        let mut enc = gif::Encoder::new(out, w, h, &[])?;
        enc.set_repeat(gif::Repeat::Infinite)?;
        Ok(Self { enc, width: w, height: h, canvas: Vec::new(), pending: None, speed: 10 })
    }

    /// 推入一帧（BGRA，自上而下）。`t` 是这一帧在动图里的时间。
    pub fn push(&mut self, bgra: &[u8], t: Duration) -> Result<(), GifError> {
        let (w, h) = (self.width as usize, self.height as usize);
        if bgra.len() < w * h * 4 {
            return Err(GifError::BadFrame);
        }
        let rgba: Vec<u8> = bgra[..w * h * 4].chunks_exact(4).flat_map(|p| [p[2], p[1], p[0], 255]).collect();
        let (x, y, bw, bh) = if self.canvas.is_empty() {
            (0, 0, w, h)
        } else {
            match diff_box(&self.canvas, &rgba, w, h) {
                Some(b) => b,
                None => return Ok(()), // 没变化：上一帧多显示一会儿
            }
        };
        let mut sub = Vec::with_capacity(bw * bh * 4);
        for row in y..y + bh {
            sub.extend_from_slice(&rgba[(row * w + x) * 4..(row * w + x + bw) * 4]);
        }
        let mut frame = gif::Frame::from_rgba_speed(bw as u16, bh as u16, &mut sub, self.speed);
        frame.left = x as u16;
        frame.top = y as u16;
        frame.dispose = gif::DisposalMethod::Keep;
        self.canvas = rgba;
        let at = centis(t);
        self.flush_pending(at)?;
        self.pending = Some(Pending { frame, at });
        Ok(())
    }

    /// 写出等待中的那一帧，它一直显示到 `until`。
    fn flush_pending(&mut self, until: u32) -> Result<(), GifError> {
        if let Some(mut p) = self.pending.take() {
            p.frame.delay = until.saturating_sub(p.at).clamp(2, u16::MAX as u32) as u16;
            self.enc.write_frame(&p.frame)?;
        }
        Ok(())
    }

    /// 写完。`end` 是录制结束的时间（最后一帧显示到这时）。
    pub fn finish(mut self, end: Duration) -> Result<W, GifError> {
        let end = centis(end);
        self.flush_pending(end)?;
        Ok(self.enc.into_inner()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, c: [u8; 4]) -> Vec<u8> {
        (0..w * h).flat_map(|_| c).collect()
    }

    #[test]
    fn diff_box_finds_changed_rect() {
        let (w, h) = (10, 8);
        let a = solid(w, h, [0, 0, 0, 255]);
        let mut b = a.clone();
        assert_eq!(diff_box(&a, &b, w, h), None);
        for (x, y) in [(2, 3), (6, 5)] {
            b[(y * w + x) * 4] = 255;
        }
        assert_eq!(diff_box(&a, &b, w, h), Some((2, 3, 5, 3)));
    }

    #[test]
    fn writes_frames_and_merges_identical_ones() {
        let (w, h) = (40usize, 30usize);
        let mut g = GifWriter::new(Vec::new(), w as u32, h as u32).unwrap();
        let red = solid(w, h, [0, 0, 255, 255]); // BGRA
        let mut blue_corner = red.clone();
        for y in 0..5 {
            for x in 0..5 {
                blue_corner[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        g.push(&red, Duration::from_millis(0)).unwrap();
        g.push(&red, Duration::from_millis(100)).unwrap(); // 一样：不写
        g.push(&red, Duration::from_millis(200)).unwrap();
        g.push(&blue_corner, Duration::from_millis(300)).unwrap();
        let bytes = g.finish(Duration::from_millis(500)).unwrap();
        assert_eq!(&bytes[..6], b"GIF89a");

        // 解码回来：两帧，第一帧显示 300 ms，第二帧只有左上角 5×5、显示 200 ms
        let mut opts = gif::DecodeOptions::new();
        opts.set_color_output(gif::ColorOutput::Indexed);
        let mut dec = opts.read_info(&bytes[..]).unwrap();
        let f1 = dec.read_next_frame().unwrap().unwrap().clone();
        assert_eq!((f1.width, f1.height, f1.delay), (40, 30, 30));
        let f2 = dec.read_next_frame().unwrap().unwrap().clone();
        assert_eq!((f2.left, f2.top, f2.width, f2.height, f2.delay), (0, 0, 5, 5, 20));
        assert!(dec.read_next_frame().unwrap().is_none());
    }
}
