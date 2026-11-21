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
        CloseClipboard, EmptyClipboard, OpenClipboard, RegisterClipboardFormatW, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};

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
