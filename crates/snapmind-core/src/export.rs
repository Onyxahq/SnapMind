//! 导出截图：裁剪、缩放、编码（PNG / JPG / WebP）、文件名模板。
//!
//! 输入是冻结帧的 BGRA 像素（GDI 原生格式），输出是可写盘或放进剪贴板的字节。
//! 标注合成（M2.3）也会接在裁剪之后、编码之前。

use std::path::{Path, PathBuf};

use chrono::{Datelike, NaiveDateTime, Timelike};
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageBuffer, ImageEncoder, Rgba};
use serde::{Deserialize, Serialize};

/// 保存格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    #[default]
    Png,
    Jpg,
    Webp,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpg => "jpg",
            Self::Webp => "webp",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("区域超出截图范围")]
    OutOfBounds,
    #[error("尺寸无效")]
    BadSize,
    #[error("编码失败：{0}")]
    Encode(String),
    #[error("写入文件失败：{0}")]
    Io(#[from] std::io::Error),
}

/// RGBA8 图像。
#[derive(Debug, Clone, PartialEq)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// 从 BGRA 截图里裁出一块，顺便转成 RGBA（不透明）。
pub fn crop_bgra(
    src: &[u8],
    src_width: u32,
    src_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> Result<RgbaImage, ExportError> {
    if width == 0 || height == 0 {
        return Err(ExportError::BadSize);
    }
    if x + width > src_width || y + height > src_height || src.len() < (src_width * src_height * 4) as usize {
        return Err(ExportError::OutOfBounds);
    }
    let mut data = Vec::with_capacity((width * height * 4) as usize);
    for row in y..y + height {
        let start = ((row * src_width + x) * 4) as usize;
        for px in src[start..start + (width * 4) as usize].chunks_exact(4) {
            data.extend_from_slice(&[px[2], px[1], px[0], 255]);
        }
    }
    Ok(RgbaImage { width, height, data })
}

/// 把标注层（RGBA，非预乘 alpha）按 source-over 叠到截图的 (dx, dy) 处。标注层必须完全落在截图内。
pub fn composite_over(base: &mut RgbaImage, top: &RgbaImage, dx: u32, dy: u32) -> Result<(), ExportError> {
    if dx + top.width > base.width || dy + top.height > base.height {
        return Err(ExportError::OutOfBounds);
    }
    let bw = base.width as usize;
    let tw = top.width as usize;
    for row in 0..top.height as usize {
        let b0 = ((dy as usize + row) * bw + dx as usize) * 4;
        let t0 = row * tw * 4;
        let brow = &mut base.data[b0..b0 + tw * 4];
        let trow = &top.data[t0..t0 + tw * 4];
        for (b, t) in brow.chunks_exact_mut(4).zip(trow.chunks_exact(4)) {
            let a = t[3] as u32;
            if a == 0 {
                continue;
            }
            if a == 255 {
                b.copy_from_slice(t);
                continue;
            }
            // 底图不透明，结果也不透明：out = top·a + base·(1−a)
            for i in 0..3 {
                b[i] = ((t[i] as u32 * a + b[i] as u32 * (255 - a) + 127) / 255) as u8;
            }
            b[3] = 255;
        }
    }
    Ok(())
}

/// 缩放到指定尺寸（尺寸相同时原样返回）。缩小用 Lanczos3 保证文字清晰。
pub fn resize(img: RgbaImage, width: u32, height: u32) -> Result<RgbaImage, ExportError> {
    if width == 0 || height == 0 {
        return Err(ExportError::BadSize);
    }
    if width == img.width && height == img.height {
        return Ok(img);
    }
    let buf: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(img.width, img.height, img.data).ok_or(ExportError::BadSize)?;
    let filter = if width < img.width { FilterType::Lanczos3 } else { FilterType::CatmullRom };
    let out = image::imageops::resize(&buf, width, height, filter);
    Ok(RgbaImage { width, height, data: out.into_raw() })
}

/// 编码。`quality` 取 1–100，只对 JPG / WebP 有效；`fast` 时 PNG 用快速压缩（剪贴板临时文件）。
pub fn encode(img: &RgbaImage, format: ImageFormat, quality: u8, fast: bool) -> Result<Vec<u8>, ExportError> {
    let q = quality.clamp(1, 100);
    let mut out = Vec::new();
    match format {
        ImageFormat::Png => {
            let compression = if fast { CompressionType::Fast } else { CompressionType::Default };
            PngEncoder::new_with_quality(&mut out, compression, PngFilter::Adaptive)
                .write_image(&img.data, img.width, img.height, ExtendedColorType::Rgba8)
                .map_err(|e| ExportError::Encode(e.to_string()))?;
        }
        ImageFormat::Jpg => {
            // JPG 没有透明通道，去掉 alpha
            let rgb: Vec<u8> = img.data.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
            JpegEncoder::new_with_quality(&mut out, q)
                .write_image(&rgb, img.width, img.height, ExtendedColorType::Rgb8)
                .map_err(|e| ExportError::Encode(e.to_string()))?;
        }
        ImageFormat::Webp => {
            let enc = webp::Encoder::from_rgba(&img.data, img.width, img.height);
            out = enc.encode(q as f32).to_vec();
        }
    }
    Ok(out)
}

/// 解码 PNG（历史里的贴图原图）为 RGBA。
pub fn decode_png(bytes: &[u8]) -> Result<RgbaImage, ExportError> {
    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| ExportError::Encode(e.to_string()))?
        .to_rgba8();
    Ok(RgbaImage { width: img.width(), height: img.height(), data: img.into_raw() })
}

/// 当前本地时间（文件名模板用）。
pub fn now_local() -> NaiveDateTime {
    chrono::Local::now().naive_local()
}

/// 默认文件名模板。
pub const DEFAULT_TEMPLATE: &str = "SnapMind_{yyyyMMdd_HHmmss}";

/// 按模板生成文件名（不含扩展名）。花括号里的 yyyy MM dd HH mm ss SSS 换成时间，
/// 其他字符原样保留；花括号外的内容原样保留。结果里不合法的文件名字符换成下划线。
pub fn render_filename(template: &str, now: NaiveDateTime) -> String {
    let tokens: [(&str, String); 7] = [
        ("yyyy", format!("{:04}", now.year())),
        ("SSS", format!("{:03}", now.and_utc().timestamp_subsec_millis())),
        ("MM", format!("{:02}", now.month())),
        ("dd", format!("{:02}", now.day())),
        ("HH", format!("{:02}", now.hour())),
        ("mm", format!("{:02}", now.minute())),
        ("ss", format!("{:02}", now.second())),
    ];
    let mut out = String::new();
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            rest = "";
            break;
        };
        let mut inner = &after[..close];
        while !inner.is_empty() {
            match tokens.iter().find(|(t, _)| inner.starts_with(t)) {
                Some((t, v)) => {
                    out.push_str(v);
                    inner = &inner[t.len()..];
                }
                None => {
                    let ch = inner.chars().next().expect("非空");
                    out.push(ch);
                    inner = &inner[ch.len_utf8()..];
                }
            }
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    sanitize_filename(&out)
}

/// 把 Windows 文件名里不允许的字符换成下划线，去掉结尾的点和空格；为空时用 SnapMind。
pub fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches(['.', ' ']);
    if trimmed.is_empty() { "SnapMind".to_string() } else { trimmed.to_string() }
}

/// 目录里已有同名文件时，在文件名后面加 (2)、(3)… 直到不重名。
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|p| !p.exists())
        .expect("总能找到不重名的文件名")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32, m: u32, s: u32, ms: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 3, 7).unwrap().and_hms_milli_opt(h, m, s, ms).unwrap()
    }

    /// 2×2 的 BGRA：红 绿 / 蓝 白
    fn sample() -> Vec<u8> {
        vec![0, 0, 255, 255, 0, 255, 0, 255, 255, 0, 0, 255, 255, 255, 255, 255]
    }

    #[test]
    fn crop_converts_bgra_to_rgba() {
        let img = crop_bgra(&sample(), 2, 2, 1, 0, 1, 2).unwrap();
        assert_eq!((img.width, img.height), (1, 2));
        assert_eq!(img.data, vec![0, 255, 0, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn crop_rejects_bad_regions() {
        assert!(matches!(crop_bgra(&sample(), 2, 2, 1, 1, 2, 1), Err(ExportError::OutOfBounds)));
        assert!(matches!(crop_bgra(&sample(), 2, 2, 0, 0, 0, 1), Err(ExportError::BadSize)));
    }

    #[test]
    fn resize_changes_size_and_keeps_same() {
        let img = crop_bgra(&sample(), 2, 2, 0, 0, 2, 2).unwrap();
        assert_eq!(resize(img.clone(), 2, 2).unwrap(), img);
        let big = resize(img, 8, 4).unwrap();
        assert_eq!((big.width, big.height, big.data.len()), (8, 4, 8 * 4 * 4));
    }

    #[test]
    fn encodes_all_formats_with_right_signatures() {
        let img = crop_bgra(&sample(), 2, 2, 0, 0, 2, 2).unwrap();
        let png = encode(&img, ImageFormat::Png, 90, true).unwrap();
        assert_eq!(&png[1..4], b"PNG");
        let jpg = encode(&img, ImageFormat::Jpg, 90, false).unwrap();
        assert_eq!(&jpg[..2], &[0xFF, 0xD8]);
        let webp = encode(&img, ImageFormat::Webp, 80, false).unwrap();
        assert_eq!(&webp[..4], b"RIFF");
        assert_eq!(&webp[8..12], b"WEBP");
    }

    #[test]
    fn lower_quality_gives_smaller_jpg() {
        // 有细节的图，质量差异才明显
        let w = 64u32;
        let src: Vec<u8> = (0..w * w).flat_map(|i| [(i * 7) as u8, (i * 13) as u8, (i * 31) as u8, 255]).collect();
        let img = crop_bgra(&src, w, w, 0, 0, w, w).unwrap();
        let hi = encode(&img, ImageFormat::Jpg, 95, false).unwrap().len();
        let lo = encode(&img, ImageFormat::Jpg, 30, false).unwrap().len();
        assert!(lo < hi, "q30={lo} 应小于 q95={hi}");
    }

    #[test]
    fn composite_blends_with_alpha() {
        let mut base = RgbaImage { width: 3, height: 1, data: vec![0, 0, 0, 255, 0, 0, 0, 255, 100, 100, 100, 255] };
        // 透明 / 不透明红 / 半透明白
        let top = RgbaImage { width: 3, height: 1, data: vec![9, 9, 9, 0, 255, 0, 0, 255, 255, 255, 255, 128] };
        composite_over(&mut base, &top, 0, 0).unwrap();
        assert_eq!(&base.data[0..4], &[0, 0, 0, 255], "完全透明不改变底图");
        assert_eq!(&base.data[4..8], &[255, 0, 0, 255], "不透明直接覆盖");
        assert_eq!(&base.data[8..12], &[178, 178, 178, 255], "半透明按比例混合");
    }

    #[test]
    fn composite_at_offset_and_bounds() {
        // 3×2 黑底，在 (1,1) 叠一个 2×1 的红
        let mut base = RgbaImage { width: 3, height: 2, data: [0, 0, 0, 255].repeat(6) };
        let top = RgbaImage { width: 2, height: 1, data: [255, 0, 0, 255].repeat(2) };
        composite_over(&mut base, &top, 1, 1).unwrap();
        assert_eq!(&base.data[0..12], &[0, 0, 0, 255].repeat(3)[..], "第一行不变");
        assert_eq!(&base.data[12..16], &[0, 0, 0, 255]);
        assert_eq!(&base.data[16..24], &[255, 0, 0, 255].repeat(2)[..]);
        assert!(composite_over(&mut base, &top, 2, 1).is_err(), "超出右边界");
    }

    #[test]
    fn filename_template() {
        let t = at(9, 5, 3, 42);
        assert_eq!(render_filename(DEFAULT_TEMPLATE, t), "SnapMind_20260307_090503");
        assert_eq!(render_filename("截图 {yyyy-MM-dd HH.mm.ss.SSS}", t), "截图 2026-03-07 09.05.03.042");
        assert_eq!(render_filename("plain", t), "plain");
        assert_eq!(render_filename("a{b", t), "a{b");
        assert_eq!(render_filename("{x}MM", t), "xMM", "花括号外的字母不替换");
    }

    #[test]
    fn filename_sanitized() {
        assert_eq!(sanitize_filename("a/b:c*?.  "), "a_b_c__");
        assert_eq!(sanitize_filename(" .. "), "SnapMind");
    }

    #[test]
    fn unique_path_appends_number() {
        let dir = tempfile::tempdir().unwrap();
        let p1 = unique_path(dir.path(), "shot", "png");
        assert_eq!(p1.file_name().unwrap(), "shot.png");
        std::fs::write(&p1, b"x").unwrap();
        let p2 = unique_path(dir.path(), "shot", "png");
        assert_eq!(p2.file_name().unwrap(), "shot (2).png");
    }
}
