//! 剪贴板：文字；截图（位图 + PNG + 文件，一次放进去，粘到哪里都合适）。
//!
//! 截图复制时同时放三种格式：
//! - `CF_DIB`：几乎所有程序都认的位图（画图、Word、微信、QQ）；
//! - `PNG`：注册格式，浏览器、Office 等优先用它，保留无损；
//! - `CF_HDROP` + `Preferred DropEffect = 复制`：资源管理器里粘贴得到一个 PNG 文件。
//!   聊天软件粘贴图片文件时会按图片发送，所以三种格式并存不会让聊天里变成发文件。

use std::path::Path;

#[derive(Debug, thiserror::Error)]
#[error("写入剪贴板失败：{0}")]
pub struct ClipboardError(String);

/// 剪贴板里的内容（贴剪贴板 Shift+F3 用）。
#[derive(Debug)]
pub enum ClipContent {
    /// RGBA8，自上而下
    Image { width: u32, height: u32, rgba: Vec<u8> },
    Text(String),
    Empty,
}

/// 读取剪贴板：优先取图片，其次取文字。
pub fn read() -> ClipContent {
    let Ok(mut cb) = arboard::Clipboard::new() else { return ClipContent::Empty };
    if let Ok(img) = cb.get_image() {
        if img.width > 0 && img.height > 0 {
            return ClipContent::Image { width: img.width as u32, height: img.height as u32, rgba: img.bytes.into_owned() };
        }
    }
    match cb.get_text() {
        Ok(t) if !t.trim().is_empty() => ClipContent::Text(t),
        _ => ClipContent::Empty,
    }
}

/// 写入纯文字。
pub fn set_text(text: &str) -> Result<(), ClipboardError> {
    let mut cb = arboard::Clipboard::new().map_err(|e| ClipboardError(e.to_string()))?;
    cb.set_text(text).map_err(|e| ClipboardError(e.to_string()))
}

/// 一张要放进剪贴板的截图。
pub struct ImageClip<'a> {
    pub width: u32,
    pub height: u32,
    /// RGBA8，自上而下
    pub rgba: &'a [u8],
    /// 已编码好的 PNG
    pub png: &'a [u8],
    /// 已写到磁盘的 PNG 文件（用于「粘贴成文件」）
    pub file: Option<&'a Path>,
}

/// 写入截图（位图 + PNG + 文件）。
pub fn set_image(clip: &ImageClip) -> Result<(), ClipboardError> {
    imp::set_image(clip)
}

/// 剪贴板当时的全部内容（每种格式的原始字节），划词时模拟复制前存下、取完词再放回去。
/// 位图句柄、图元文件这类 GDI 格式存不下来，跳过（系统会从 CF_DIB 重新合成位图）。
#[derive(Debug, Default, Clone)]
pub struct Snapshot {
    pub formats: Vec<(u32, Vec<u8>)>,
}

/// 存下剪贴板的全部内容。打不开剪贴板时返回 None。
pub fn snapshot() -> Option<Snapshot> {
    imp::snapshot()
}

/// 放回存下的内容（原来是空的就清空）。
pub fn restore(s: &Snapshot) -> Result<(), ClipboardError> {
    imp::restore(s)
}

/// 系统会互相合成的格式族：文字（CF_UNICODETEXT / CF_TEXT / CF_OEMTEXT）、位图
/// （CF_BITMAP / CF_DIB / CF_DIBV5）。原程序放进去的那个排在前面，合成出来的排在后面。
/// 每族只留原来那个——连合成的一起放回去，读的程序可能取到转换过一次的数据（位图会错位）。
/// 原来那个是存不下来的 GDI 位图（CF_BITMAP）时，留系统合成的 CF_DIB，让系统再从它合成其余的。
pub fn without_synthesized(formats: Vec<(u32, Vec<u8>)>, order: &[u32]) -> Vec<(u32, Vec<u8>)> {
    const FAMILIES: [[u32; 3]; 2] = [[13, 1, 7], [2, 8, 17]];
    let mut drop: Vec<u32> = Vec::new();
    for fam in FAMILIES {
        let Some(&first) = order.iter().find(|f| fam.contains(f)) else { continue };
        let keep = if first == 2 {
            [8, 17].into_iter().find(|f| formats.iter().any(|(g, _)| g == f)).unwrap_or(0)
        } else {
            first
        };
        drop.extend(fam.iter().copied().filter(|&f| f != keep));
    }
    formats.into_iter().filter(|(f, _)| !drop.contains(f)).collect()
}

/// 剪贴板的序号：内容每变一次加一，用来判断模拟复制有没有生效。
pub fn sequence() -> u32 {
    imp::sequence()
}

/// 读出剪贴板里的文字（没有文字时为 None）。
pub fn text() -> Option<String> {
    arboard::Clipboard::new().ok()?.get_text().ok()
}

/// CF_DIB 数据：BITMAPINFOHEADER + 自下而上的 32 位 BGRA 像素。
pub fn dib_bytes(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    let row = (width * 4) as usize;
    let mut out = Vec::with_capacity(40 + rgba.len());
    out.extend_from_slice(&40u32.to_le_bytes());
    out.extend_from_slice(&(width as i32).to_le_bytes());
    out.extend_from_slice(&(height as i32).to_le_bytes()); // 正数 = 自下而上，兼容性最好
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes()); // BI_RGB
    out.extend_from_slice(&(rgba.len() as u32).to_le_bytes());
    out.extend_from_slice(&[0u8; 16]); // 分辨率与调色板
    for y in (0..height as usize).rev() {
        for p in rgba[y * row..(y + 1) * row].chunks_exact(4) {
            out.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
        }
    }
    out
}

/// CF_HDROP 数据：DROPFILES 头（宽字符）+ 以双零结尾的路径列表。
pub fn hdrop_bytes(path: &Path) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&20u32.to_le_bytes()); // pFiles：路径从第 20 字节开始
    out.extend_from_slice(&[0u8; 8]); // pt
    out.extend_from_slice(&0u32.to_le_bytes()); // fNC
    out.extend_from_slice(&1u32.to_le_bytes()); // fWide
    for u in path.as_os_str().to_string_lossy().encode_utf16().chain([0, 0]) {
        out.extend_from_slice(&u.to_le_bytes());
    }
    out
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::core::w;
    use windows::Win32::Foundation::{GlobalFree, HANDLE, HGLOBAL};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, EnumClipboardFormats, GetClipboardData, GetClipboardSequenceNumber,
        OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};

    /// 不是 HGLOBAL 的格式（GDI 句柄、所有者绘制），存不下来
    const HANDLE_FORMATS: [u32; 8] = [2, 3, 9, 14, 0x80, 0x82, 0x83, 0x8E];

    fn open_retry() -> bool {
        for _ in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        false
    }

    pub fn sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    pub fn snapshot() -> Option<Snapshot> {
        if !open_retry() {
            return None;
        }
        let _guard = Open;
        let mut out = Snapshot::default();
        let mut fmt = 0u32;
        loop {
            fmt = unsafe { EnumClipboardFormats(fmt) };
            if fmt == 0 {
                break;
            }
            if HANDLE_FORMATS.contains(&fmt) {
                continue;
            }
            unsafe {
                let Ok(h) = GetClipboardData(fmt) else { continue };
                let mem = HGLOBAL(h.0);
                let size = GlobalSize(mem);
                let p = GlobalLock(mem) as *const u8;
                if p.is_null() || size == 0 {
                    continue;
                }
                let bytes = std::slice::from_raw_parts(p, size).to_vec();
                let _ = GlobalUnlock(mem);
                out.formats.push((fmt, bytes));
            }
        }
        let order = enumerated();
        out.formats = without_synthesized(out.formats, &order);
        Some(out)
    }

    /// 剪贴板里所有格式的编号，按枚举顺序（调用方已打开剪贴板）。
    fn enumerated() -> Vec<u32> {
        let mut v = Vec::new();
        let mut fmt = 0u32;
        loop {
            fmt = unsafe { EnumClipboardFormats(fmt) };
            if fmt == 0 {
                return v;
            }
            v.push(fmt);
        }
    }

    pub fn restore(s: &Snapshot) -> Result<(), ClipboardError> {
        if !open_retry() {
            return Err(ClipboardError("剪贴板被其他程序占用".into()));
        }
        let _guard = Open;
        unsafe { EmptyClipboard() }.map_err(|e| ClipboardError(format!("EmptyClipboard：{e}")))?;
        for (fmt, bytes) in &s.formats {
            put(*fmt, bytes)?;
        }
        Ok(())
    }

    const CF_DIB: u32 = 8;
    const CF_HDROP: u32 = 15;

    /// 打开剪贴板的守卫：任何情况下都会关掉。
    struct Open;
    impl Drop for Open {
        fn drop(&mut self) {
            unsafe {
                let _ = CloseClipboard();
            }
        }
    }

    fn put(format: u32, bytes: &[u8]) -> Result<(), ClipboardError> {
        unsafe {
            let mem: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, bytes.len())
                .map_err(|e| ClipboardError(format!("GlobalAlloc：{e}")))?;
            let p = GlobalLock(mem) as *mut u8;
            if p.is_null() {
                let _ = GlobalFree(Some(mem));
                return Err(ClipboardError("GlobalLock 失败".into()));
            }
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
            let _ = GlobalUnlock(mem);
            // 成功后内存归系统所有，不能再释放
            if SetClipboardData(format, Some(HANDLE(mem.0))).is_err() {
                let _ = GlobalFree(Some(mem));
                return Err(ClipboardError(format!("SetClipboardData({format}) 失败")));
            }
        }
        Ok(())
    }

    pub fn set_image(clip: &ImageClip) -> Result<(), ClipboardError> {
        // 别的程序可能正占着剪贴板，稍等重试
        let mut opened = false;
        for _ in 0..10 {
            if unsafe { OpenClipboard(None) }.is_ok() {
                opened = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !opened {
            return Err(ClipboardError("剪贴板被其他程序占用".into()));
        }
        let _guard = Open;
        unsafe { EmptyClipboard() }.map_err(|e| ClipboardError(format!("EmptyClipboard：{e}")))?;
        put(CF_DIB, &dib_bytes(clip.width, clip.height, clip.rgba))?;
        let png = unsafe { RegisterClipboardFormatW(w!("PNG")) };
        if png != 0 {
            put(png, clip.png)?;
        }
        if let Some(file) = clip.file {
            put(CF_HDROP, &hdrop_bytes(file))?;
            let effect = unsafe { RegisterClipboardFormatW(w!("Preferred DropEffect")) };
            if effect != 0 {
                put(effect, &1u32.to_le_bytes())?; // DROPEFFECT_COPY：粘贴是复制而不是移动
            }
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    // Linux（M12）：先只存文字
    pub fn sequence() -> u32 {
        0
    }
    pub fn snapshot() -> Option<Snapshot> {
        Some(Snapshot { formats: super::text().map(|t| vec![(13, t.into_bytes())]).unwrap_or_default() })
    }
    pub fn restore(s: &Snapshot) -> Result<(), ClipboardError> {
        match s.formats.first() {
            Some((_, b)) => super::set_text(&String::from_utf8_lossy(b)),
            None => Ok(()),
        }
    }

    pub fn set_image(clip: &ImageClip) -> Result<(), ClipboardError> {
        let mut cb = arboard::Clipboard::new().map_err(|e| ClipboardError(e.to_string()))?;
        cb.set_image(arboard::ImageData {
            width: clip.width as usize,
            height: clip.height as usize,
            bytes: std::borrow::Cow::Borrowed(clip.rgba),
        })
        .map_err(|e| ClipboardError(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dib_is_bottom_up_bgra() {
        // 1×2：上红下蓝
        let rgba = [255, 0, 0, 255, 0, 0, 255, 255];
        let d = dib_bytes(1, 2, &rgba);
        assert_eq!(d.len(), 40 + 8);
        assert_eq!(i32::from_le_bytes(d[8..12].try_into().unwrap()), 2);
        assert_eq!(&d[40..44], &[255, 0, 0, 255], "第一行是最下面的蓝色（BGRA）");
        assert_eq!(&d[44..48], &[0, 0, 255, 255]);
    }

    #[test]
    fn keeps_only_original_of_synthesized_families() {
        let f = |ids: &[u32]| ids.iter().map(|&i| (i, vec![i as u8])).collect::<Vec<_>>();
        let ids = |v: Vec<(u32, Vec<u8>)>| v.into_iter().map(|(i, _)| i).collect::<Vec<_>>();
        // .NET 放了 CF_UNICODETEXT 和 GDI 位图：合成的 CF_TEXT / OEMTEXT / DIBV5 去掉，位图留 CF_DIB
        let order = [49161, 13, 2, 50089, 16, 1, 7, 8, 17];
        let saved = f(&[49161, 13, 50089, 16, 1, 7, 8, 17]); // CF_BITMAP 本来就存不下
        assert_eq!(ids(without_synthesized(saved, &order)), vec![49161, 13, 50089, 16, 8]);
        // 程序直接放的 CF_DIB 和 PNG：DIBV5 是合成的
        let order = [8, 49300, 2, 17];
        assert_eq!(ids(without_synthesized(f(&[8, 49300, 17]), &order)), vec![8, 49300]);
        // 只有 ANSI 文字的老程序：留 CF_TEXT
        assert_eq!(ids(without_synthesized(f(&[1, 16, 13, 7]), &[1, 16, 13, 7])), vec![1, 16]);
    }

    #[test]
    fn hdrop_layout() {
        let d = hdrop_bytes(Path::new("C:\\a.png"));
        assert_eq!(u32::from_le_bytes(d[0..4].try_into().unwrap()), 20);
        assert_eq!(u32::from_le_bytes(d[16..20].try_into().unwrap()), 1);
        let chars: Vec<u16> = d[20..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        assert_eq!(String::from_utf16_lossy(&chars[..8]), "C:\\a.png");
        assert_eq!(&chars[8..], &[0, 0]);
    }

    /// 真的写一次剪贴板（会覆盖当前剪贴板内容）。
    #[cfg(windows)]
    #[test]
    #[ignore]
    fn writes_image_to_clipboard() {
        let rgba = [255u8, 0, 0, 255].repeat(4);
        let clip = ImageClip { width: 2, height: 2, rgba: &rgba, png: b"\x89PNG", file: None };
        set_image(&clip).unwrap();
    }
}
