//! 未压缩 BMP 封装。
//!
//! 截图要尽快交给 WebView 显示：PNG 编码一张 4K 图要上百毫秒，而 BMP 只是在像素前面
//! 加 54 字节文件头，浏览器解码也很快。像素按 32 位 BGRA、自上而下排列，
//! 正好是 GDI DIB 的原生格式，抓屏后直接拷贝即可。

/// 文件头（14 字节）+ 信息头（40 字节）。
pub const HEADER_LEN: usize = 54;

/// 生成 32 位、自上而下（高度取负）的 BMP 文件头。
pub fn header(width: u32, height: u32) -> [u8; HEADER_LEN] {
    let pixels = width as u64 * height as u64 * 4;
    let file_size = (HEADER_LEN as u64 + pixels).min(u32::MAX as u64) as u32;
    let mut h = [0u8; HEADER_LEN];
    // BITMAPFILEHEADER
    h[0..2].copy_from_slice(b"BM");
    h[2..6].copy_from_slice(&file_size.to_le_bytes());
    h[10..14].copy_from_slice(&(HEADER_LEN as u32).to_le_bytes());
    // BITMAPINFOHEADER
    h[14..18].copy_from_slice(&40u32.to_le_bytes());
    h[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    h[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
    h[26..28].copy_from_slice(&1u16.to_le_bytes()); // planes
    h[28..30].copy_from_slice(&32u16.to_le_bytes()); // bpp
    // 压缩方式 BI_RGB = 0，其余字段为 0
    h
}

/// 一帧截图：BMP 文件的完整字节（文件头 + BGRA 像素），可直接作为 HTTP 响应体。
#[derive(Clone)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bmp: Vec<u8>,
}

impl Frame {
    /// 从 BGRA 像素构造。透明度统一置为不透明：GDI 抓到的 alpha 是不确定的，
    /// 有些浏览器会把非零 alpha 当真，导致截图半透明。
    pub fn from_bgra(width: u32, height: u32, bgra: &[u8]) -> Self {
        let mut bmp = Vec::with_capacity(HEADER_LEN + bgra.len());
        bmp.extend_from_slice(&header(width, height));
        bmp.extend_from_slice(bgra);
        for px in bmp[HEADER_LEN..].chunks_exact_mut(4) {
            px[3] = 255;
        }
        Self { width, height, bmp }
    }

    /// 像素部分（BGRA，自上而下）。
    pub fn pixels(&self) -> &[u8] {
        &self.bmp[HEADER_LEN..]
    }

    /// 取某个像素的 (r, g, b)。越界返回 None。
    pub fn rgb_at(&self, x: u32, y: u32) -> Option<(u8, u8, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = ((y * self.width + x) * 4) as usize;
        let p = &self.pixels()[i..i + 4];
        Some((p[2], p[1], p[0]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_fields() {
        let h = header(3, 2);
        assert_eq!(&h[0..2], b"BM");
        assert_eq!(u32::from_le_bytes(h[2..6].try_into().unwrap()), 54 + 3 * 2 * 4);
        assert_eq!(u32::from_le_bytes(h[10..14].try_into().unwrap()), 54);
        assert_eq!(i32::from_le_bytes(h[18..22].try_into().unwrap()), 3);
        assert_eq!(i32::from_le_bytes(h[22..26].try_into().unwrap()), -2, "自上而下");
        assert_eq!(u16::from_le_bytes(h[28..30].try_into().unwrap()), 32);
    }

    #[test]
    fn frame_forces_opaque_and_reads_rgb() {
        // 2×1：第一个像素 BGRA = (10,20,30,0)，第二个 (1,2,3,7)
        let f = Frame::from_bgra(2, 1, &[10, 20, 30, 0, 1, 2, 3, 7]);
        assert_eq!(f.bmp.len(), HEADER_LEN + 8);
        assert_eq!(f.pixels(), &[10, 20, 30, 255, 1, 2, 3, 255]);
        assert_eq!(f.rgb_at(0, 0), Some((30, 20, 10)));
        assert_eq!(f.rgb_at(1, 0), Some((3, 2, 1)));
        assert_eq!(f.rgb_at(2, 0), None);
    }
}
