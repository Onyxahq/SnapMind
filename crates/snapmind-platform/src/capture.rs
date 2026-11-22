//! 截屏：把虚拟桌面上的一块矩形抓成一帧。
//!
//! Windows 用 GDI BitBlt（带 CAPTUREBLT，分层窗口也能抓到）。进程是「每显示器 DPI 感知」，
//! 所以坐标和像素都是物理像素，高 DPI 下不会模糊或错位。

use crate::bmp::Frame;
use crate::Rect;

#[derive(Debug, thiserror::Error)]
pub enum CaptureError {
    #[error("截屏区域为空")]
    Empty,
    #[error("截屏失败：{0}")]
    Os(String),
    #[error("当前平台暂不支持截屏")]
    Unsupported,
}

/// 抓取一块矩形。
pub fn capture_rect(r: Rect) -> Result<Frame, CaptureError> {
    if r.width == 0 || r.height == 0 {
        return Err(CaptureError::Empty);
    }
    imp::capture_rect(r, false)
}

/// 抓取一块矩形，并把鼠标指针画上去（录屏用；截图本身不带指针）。
pub fn capture_rect_with_cursor(r: Rect) -> Result<Frame, CaptureError> {
    if r.width == 0 || r.height == 0 {
        return Err(CaptureError::Empty);
    }
    imp::capture_rect(r, true)
}

/// 并行抓取多块矩形（每个显示器一块），结果顺序与输入一致。
pub fn capture_all(rects: &[Rect]) -> Vec<Result<Frame, CaptureError>> {
    std::thread::scope(|s| {
        let handles: Vec<_> = rects.iter().map(|&r| s.spawn(move || capture_rect(r))).collect();
        handles
            .into_iter()
            .map(|h| h.join().unwrap_or_else(|_| Err(CaptureError::Os("截屏线程崩溃".into()))))
            .collect()
    })
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ffi::c_void;
    use windows::Win32::Graphics::Gdi::*;
    use windows::Win32::UI::WindowsAndMessaging::{
        DrawIconEx, GetCursorInfo, GetIconInfo, CURSORINFO, CURSOR_SHOWING, DI_NORMAL, ICONINFO,
    };

    /// 把当前鼠标指针画到内存 DC 上（坐标相对抓取区域左上角）。
    unsafe fn draw_cursor(mem: HDC, r: Rect) {
        let mut ci = CURSORINFO { cbSize: std::mem::size_of::<CURSORINFO>() as u32, ..Default::default() };
        unsafe {
            if GetCursorInfo(&mut ci).is_err() || ci.flags.0 & CURSOR_SHOWING.0 == 0 || ci.hCursor.is_invalid() {
                return;
            }
            let mut ii = ICONINFO::default();
            let (hx, hy) = if GetIconInfo(ci.hCursor.into(), &mut ii).is_ok() {
                // GetIconInfo 交出两张位图，用完要删掉
                if !ii.hbmMask.is_invalid() {
                    let _ = DeleteObject(ii.hbmMask.into());
                }
                if !ii.hbmColor.is_invalid() {
                    let _ = DeleteObject(ii.hbmColor.into());
                }
                (ii.xHotspot as i32, ii.yHotspot as i32)
            } else {
                (0, 0)
            };
            let x = ci.ptScreenPos.x - hx - r.x;
            let y = ci.ptScreenPos.y - hy - r.y;
            let _ = DrawIconEx(mem, x, y, ci.hCursor.into(), 0, 0, 0, None, DI_NORMAL);
        }
    }

    /// 释放 GDI 资源的守卫，任何一步出错都不会泄漏句柄。
    struct Gdi {
        screen: HDC,
        mem: HDC,
        bitmap: HBITMAP,
        old: HGDIOBJ,
    }

    impl Drop for Gdi {
        fn drop(&mut self) {
            unsafe {
                if !self.old.is_invalid() {
                    SelectObject(self.mem, self.old);
                }
                if !self.bitmap.is_invalid() {
                    let _ = DeleteObject(self.bitmap.into());
                }
                if !self.mem.is_invalid() {
                    let _ = DeleteDC(self.mem);
                }
                if !self.screen.is_invalid() {
                    ReleaseDC(None, self.screen);
                }
            }
        }
    }

    pub fn capture_rect(r: Rect, cursor: bool) -> Result<Frame, CaptureError> {
        let (w, h) = (r.width as i32, r.height as i32);
        unsafe {
            let mut g = Gdi {
                screen: GetDC(None),
                mem: HDC::default(),
                bitmap: HBITMAP::default(),
                old: HGDIOBJ::default(),
            };
            if g.screen.is_invalid() {
                return Err(CaptureError::Os("GetDC 失败".into()));
            }
            g.mem = CreateCompatibleDC(Some(g.screen));
            if g.mem.is_invalid() {
                return Err(CaptureError::Os("CreateCompatibleDC 失败".into()));
            }
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: w,
                    biHeight: -h, // 自上而下
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits: *mut c_void = std::ptr::null_mut();
            g.bitmap = CreateDIBSection(Some(g.screen), &info, DIB_RGB_COLORS, &mut bits, None, 0)
                .map_err(|e| CaptureError::Os(format!("CreateDIBSection：{e}")))?;
            g.old = SelectObject(g.mem, g.bitmap.into());
            BitBlt(g.mem, 0, 0, w, h, Some(g.screen), r.x, r.y, SRCCOPY | CAPTUREBLT)
                .map_err(|e| CaptureError::Os(format!("BitBlt：{e}")))?;
            if cursor {
                draw_cursor(g.mem, r);
            }
            let _ = GdiFlush();
            let len = r.width as usize * r.height as usize * 4;
            let pixels = std::slice::from_raw_parts(bits as *const u8, len);
            Ok(Frame::from_bgra(r.width, r.height, pixels))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    pub fn capture_rect(_r: Rect, _cursor: bool) -> Result<Frame, CaptureError> {
        Err(CaptureError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rect_is_rejected() {
        let r = Rect { x: 0, y: 0, width: 0, height: 10 };
        assert!(matches!(capture_rect(r), Err(CaptureError::Empty)));
    }

    /// 在有桌面的 Windows 上真的抓一小块，检查尺寸和格式。
    #[cfg(windows)]
    #[test]
    fn captures_a_small_region() {
        let r = Rect { x: 0, y: 0, width: 64, height: 32 };
        let f = capture_rect(r).expect("截屏应当成功");
        assert_eq!((f.width, f.height), (64, 32));
        assert_eq!(f.bmp.len(), crate::bmp::HEADER_LEN + 64 * 32 * 4);
        assert!(f.pixels().chunks_exact(4).all(|p| p[3] == 255));
    }

    #[cfg(windows)]
    #[test]
    fn capture_all_keeps_order() {
        let rects = [
            Rect { x: 0, y: 0, width: 8, height: 4 },
            Rect { x: 8, y: 0, width: 16, height: 2 },
        ];
        let out = capture_all(&rects);
        assert_eq!(out[0].as_ref().unwrap().width, 8);
        assert_eq!(out[1].as_ref().unwrap().width, 16);
    }
}
