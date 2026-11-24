//! 截屏：把虚拟桌面上的一块矩形抓成一帧。
//!
//! Windows 用 GDI BitBlt（带 CAPTUREBLT，分层窗口也能抓到）。进程是「每显示器 DPI 感知」，
//! 所以坐标和像素都是物理像素，高 DPI 下不会模糊或错位。
//! macOS 用 ScreenCaptureKit（15.2+ 直接按区域截，更早的按显示器截）。

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

/// 录屏的连续取帧：按帧率不停地拿「这块矩形现在的样子」（带鼠标指针）。
///
/// Windows 每次 `frame` 现抓一帧（GDI 很快）；macOS 开一个 ScreenCaptureKit 的视频流，系统在画面
/// 变化时推送新帧，`frame` 拿最新的一帧——画面没变时重复上一帧，视频时长照常。
pub struct Stream {
    inner: imp::Stream,
}

impl Stream {
    pub fn start(r: Rect, fps: u32) -> Result<Self, CaptureError> {
        if r.width == 0 || r.height == 0 {
            return Err(CaptureError::Empty);
        }
        Ok(Self { inner: imp::Stream::start(r, fps.max(1))? })
    }

    /// 当前画面（宽高与 `start` 时的矩形一致，带指针）。
    pub fn frame(&mut self) -> Result<Frame, CaptureError> {
        self.inner.frame()
    }
}

/// 抓取一块矩形，并把鼠标指针画上去（录屏用；截图本身不带指针）。
pub fn capture_rect_with_cursor(r: Rect) -> Result<Frame, CaptureError> {
    if r.width == 0 || r.height == 0 {
        return Err(CaptureError::Empty);
    }
    imp::capture_rect(r, true)
}

/// 有没有截屏权限。只有 macOS 需要用户授权（系统设置 → 隐私与安全性 → 屏幕录制），其他平台总是 true。
pub fn has_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        objc2_core_graphics::CGPreflightScreenCaptureAccess()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// 申请截屏权限。系统只在第一次弹出授权提示；`open_settings` 为 true 时，已经提示过还没授权就直接打开
/// 「屏幕录制」设置页让用户去勾选。授权后要重启应用才生效（系统的规定）。
pub fn request_permission(open_settings: bool) {
    #[cfg(not(target_os = "macos"))]
    let _ = open_settings;
    #[cfg(target_os = "macos")]
    if !objc2_core_graphics::CGRequestScreenCaptureAccess() && open_settings {
        let _ = std::process::Command::new("open")
            .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture")
            .spawn();
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::{main_display_filter, pixel_to_point, scale_at_point};

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

    /// 每次现抓一帧（GDI 抓一块矩形只要几毫秒）。
    pub struct Stream(Rect);
    impl Stream {
        pub fn start(r: Rect, _fps: u32) -> Result<Self, CaptureError> {
            Ok(Self(r))
        }
        pub fn frame(&mut self) -> Result<Frame, CaptureError> {
            capture_rect(self.0, true)
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

/// macOS：ScreenCaptureKit 的截图接口（macOS 14 起）。旧的 CGWindowListCreateImage 从 macOS 15 起已不可用。
///
/// 坐标：Tauri（tao）在 macOS 上把「点」乘以**该显示器自己的**缩放倍数当作物理像素，所以每个显示器
/// 的矩形除以它自己的倍数，就回到系统的全局点坐标（主显示器左上角为原点，y 向下）。
#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::rc::Retained;
    use objc2::AnyThread;
    use objc2_core_foundation::{CGPoint, CGRect, CGSize};
    use objc2_core_graphics::{
        CGDirectDisplayID, CGDisplayBounds, CGDisplayCopyDisplayMode, CGDisplayMode, CGDisplayPixelsWide,
        CGGetActiveDisplayList, CGImage,
    };
    use dispatch2::DispatchQueue;
    use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
    use objc2::{define_class, msg_send, DefinedClass};
    use objc2_core_media::{CMSampleBuffer, CMTime, CMTimeFlags};
    use objc2_foundation::{NSArray, NSError};
    use objc2_screen_capture_kit::{SCDisplay, SCStream, SCStreamOutput, SCStreamOutputType};
    use std::sync::{Arc, Mutex};
    use objc2_screen_capture_kit::{
        SCCaptureResolutionType, SCContentFilter, SCScreenshotManager, SCShareableContent,
        SCStreamConfiguration,
    };
    use std::ffi::c_void;
    use std::sync::mpsc;
    use std::time::Duration;

    const TIMEOUT: Duration = Duration::from_secs(3);

    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGBitmapContextCreate(
            data: *mut c_void,
            width: usize,
            height: usize,
            bits_per_component: usize,
            bytes_per_row: usize,
            space: *const c_void,
            bitmap_info: u32,
        ) -> *mut c_void;
        fn CGContextDrawImage(c: *mut c_void, rect: CGRect, image: *const c_void);
        fn CGContextRelease(c: *mut c_void);
        fn CGImageGetColorSpace(image: *const c_void) -> *const c_void;
        fn CGColorSpaceCreateDeviceRGB() -> *const c_void;
        fn CGColorSpaceRelease(space: *const c_void);
        fn CGImageGetWidth(image: *const c_void) -> usize;
    }
    /// kCGImageAlphaPremultipliedFirst | kCGBitmapByteOrder32Little：内存里是 B G R A
    const BGRA: u32 = 2 | (2 << 12);

    /// 显示器的缩放倍数（像素 / 点）。
    fn scale_of(id: u32) -> f64 {
        let points = CGDisplayPixelsWide(id) as f64;
        let mode = CGDisplayCopyDisplayMode(id);
        let pixels = CGDisplayMode::pixel_width(mode.as_deref()) as f64;
        if points > 0.0 && pixels > 0.0 { pixels / points } else { 1.0 }
    }

    fn shareable_content() -> Result<Retained<SCShareableContent>, CaptureError> {
        let (tx, rx) = mpsc::channel();
        let block = block2::RcBlock::new(move |content: *mut SCShareableContent, err: *mut NSError| {
            // SAFETY: 回调给出的对象在回调期间有效，retain 一份带出去
            let r = match unsafe { Retained::retain(content) } {
                Some(c) => Ok(c),
                None => Err(unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string()).unwrap_or_default()),
            };
            let _ = tx.send(r);
        });
        unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&block) };
        match rx.recv_timeout(TIMEOUT) {
            Ok(Ok(c)) => Ok(c),
            // 没有「屏幕录制」权限时系统在这里报错
            Ok(Err(e)) => Err(CaptureError::Os(format!("读取可截屏内容失败（是否已在「系统设置 → 隐私与安全性 → 屏幕录制」里允许 SnapMind？）：{e}"))),
            Err(_) => Err(CaptureError::Os("读取可截屏内容超时".into())),
        }
    }

    /// ScreenCaptureKit 视频流：系统在画面变化时把新帧推到我们的队列里，存成最新一帧。
    pub struct Stream {
        stream: Retained<SCStream>,
        latest: Arc<Mutex<Option<Frame>>>,
        last: Option<Frame>,
        // 输出对象要活到流停下为止
        _output: Retained<StreamOutput>,
    }
    // SAFETY: SCStream 可以在任意线程启停；帧通过 Mutex 交接。
    unsafe impl Send for Stream {}

    pub struct OutputIvars {
        latest: Arc<Mutex<Option<Frame>>>,
        width: u32,
        height: u32,
    }

    define_class!(
        // SAFETY: NSObject 没有子类化要求；StreamOutput 不实现 Drop。
        #[unsafe(super(NSObject))]
        #[name = "SnapMindVideoOutput"]
        #[ivars = OutputIvars]
        pub struct StreamOutput;

        unsafe impl NSObjectProtocol for StreamOutput {}

        unsafe impl SCStreamOutput for StreamOutput {
            #[unsafe(method(stream:didOutputSampleBuffer:ofType:))]
            fn did_output(&self, _stream: &SCStream, sample: &CMSampleBuffer, kind: SCStreamOutputType) {
                if kind != SCStreamOutputType::Screen {
                    return;
                }
                let iv = self.ivars();
                // 画面没变时系统也会送「空闲」样本，里面没有图像，跳过
                if let Some(f) = unsafe { frame_from_sample(sample, iv.width, iv.height) } {
                    *iv.latest.lock().expect("录屏帧锁") = Some(f);
                }
            }
        }
    );

    impl StreamOutput {
        fn new(latest: Arc<Mutex<Option<Frame>>>, width: u32, height: u32) -> Retained<Self> {
            let this = Self::alloc().set_ivars(OutputIvars { latest, width, height });
            unsafe { msg_send![super(this), init] }
        }
    }

    #[link(name = "CoreMedia", kind = "framework")]
    unsafe extern "C" {
        fn CMSampleBufferGetImageBuffer(sample: &CMSampleBuffer) -> *mut c_void;
    }
    #[link(name = "CoreVideo", kind = "framework")]
    unsafe extern "C" {
        fn CVPixelBufferLockBaseAddress(buf: *mut c_void, flags: u64) -> i32;
        fn CVPixelBufferUnlockBaseAddress(buf: *mut c_void, flags: u64) -> i32;
        fn CVPixelBufferGetBaseAddress(buf: *mut c_void) -> *mut u8;
        fn CVPixelBufferGetBytesPerRow(buf: *mut c_void) -> usize;
        fn CVPixelBufferGetWidth(buf: *mut c_void) -> usize;
        fn CVPixelBufferGetHeight(buf: *mut c_void) -> usize;
    }
    /// kCVPixelBufferLock_ReadOnly
    const READ_ONLY: u64 = 1;

    /// 把样本里的 BGRA 图像按行拷成 w×h 的帧（系统给的行宽可能有对齐填充；尺寸不对就不要）。
    unsafe fn frame_from_sample(sample: &CMSampleBuffer, w: u32, h: u32) -> Option<Frame> {
        unsafe {
            let buf = CMSampleBufferGetImageBuffer(sample);
            if buf.is_null() || CVPixelBufferGetWidth(buf) < w as usize || CVPixelBufferGetHeight(buf) < h as usize {
                return None;
            }
            CVPixelBufferLockBaseAddress(buf, READ_ONLY);
            let base = CVPixelBufferGetBaseAddress(buf);
            let stride = CVPixelBufferGetBytesPerRow(buf);
            let row = w as usize * 4;
            let mut bgra = vec![0u8; row * h as usize];
            if !base.is_null() {
                for y in 0..h as usize {
                    std::ptr::copy_nonoverlapping(base.add(y * stride), bgra.as_mut_ptr().add(y * row), row);
                }
            }
            CVPixelBufferUnlockBaseAddress(buf, READ_ONLY);
            (!base.is_null()).then(|| Frame::from_bgra(w, h, &bgra))
        }
    }

    /// 整块主显示器的内容过滤器（录系统声音用：声音不分显示器，随便挂一块屏就行）。
    pub(crate) fn main_display_filter() -> Result<Retained<SCContentFilter>, CaptureError> {
        let content = shareable_content()?;
        let displays = unsafe { content.displays() };
        let display = displays.iter().next().ok_or_else(|| CaptureError::Os("没有显示器".into()))?;
        Ok(unsafe { SCContentFilter::initWithDisplay_excludingWindows(SCContentFilter::alloc(), &display, &NSArray::new()) })
    }

    /// 矩形所在的显示器，以及矩形在那块显示器里的位置（点）。
    fn display_for(r: Rect) -> Result<(Retained<SCDisplay>, CGRect), CaptureError> {
        let content = shareable_content()?;
        let displays = unsafe { content.displays() };
        let Some(s) = scale_at(r) else { return Err(CaptureError::Os("找不到录屏区域所在的显示器".into())) };
        let (px, py) = (r.x as f64 / s, r.y as f64 / s);
        let display = displays.iter().find(|d| {
            let f = unsafe { d.frame() };
            px >= f.origin.x - 0.5 && py >= f.origin.y - 0.5 && px < f.origin.x + f.size.width && py < f.origin.y + f.size.height
        });
        let Some(display) = display else { return Err(CaptureError::Os("找不到录屏区域所在的显示器".into())) };
        let f = unsafe { display.frame() };
        let source = CGRect::new(
            CGPoint::new(px - f.origin.x, py - f.origin.y),
            CGSize::new(r.width as f64 / s, r.height as f64 / s),
        );
        Ok((display, source))
    }

    impl Stream {
        pub fn start(r: Rect, fps: u32) -> Result<Self, CaptureError> {
            let (display, source) = display_for(r)?;
            let filter = unsafe {
                SCContentFilter::initWithDisplay_excludingWindows(SCContentFilter::alloc(), &display, &NSArray::new())
            };
            let config = unsafe { SCStreamConfiguration::new() };
            unsafe {
                config.setSourceRect(source);
                config.setWidth(r.width as usize);
                config.setHeight(r.height as usize);
                config.setShowsCursor(true);
                config.setPixelFormat(u32::from_be_bytes(*b"BGRA"));
                config.setMinimumFrameInterval(CMTime {
                    value: 1,
                    timescale: fps as i32,
                    flags: CMTimeFlags::Valid,
                    epoch: 0,
                });
                config.setQueueDepth(5);
            }
            let latest = Arc::new(Mutex::new(None));
            let output = StreamOutput::new(latest.clone(), r.width, r.height);
            let stream = unsafe { SCStream::initWithFilter_configuration_delegate(SCStream::alloc(), &filter, &config, None) };
            let queue = DispatchQueue::new("snapmind.record", None);
            unsafe {
                stream.addStreamOutput_type_sampleHandlerQueue_error(
                    ProtocolObject::from_ref(&*output),
                    SCStreamOutputType::Screen,
                    Some(&queue),
                )
            }
            .map_err(|e| CaptureError::Os(format!("ScreenCaptureKit：{}", e.localizedDescription())))?;
            let (tx, rx) = mpsc::channel();
            let block = block2::RcBlock::new(move |err: *mut NSError| {
                let _ = tx.send(unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string()));
            });
            unsafe { stream.startCaptureWithCompletionHandler(Some(&block)) };
            match rx.recv_timeout(TIMEOUT) {
                Ok(None) => {}
                Ok(Some(e)) => return Err(CaptureError::Os(format!("ScreenCaptureKit：{e}"))),
                Err(_) => return Err(CaptureError::Os("启动录屏流超时".into())),
            }
            Ok(Self { stream, latest, last: None, _output: output })
        }

        pub fn frame(&mut self) -> Result<Frame, CaptureError> {
            // 刚开始时系统还没送来第一帧：最多等一会儿
            let t0 = std::time::Instant::now();
            loop {
                if let Some(f) = self.latest.lock().expect("录屏帧锁").take() {
                    self.last = Some(f);
                    break;
                }
                if self.last.is_some() || t0.elapsed() > TIMEOUT {
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            self.last.clone().ok_or_else(|| CaptureError::Os("录屏流没有送来画面".into()))
        }
    }

    impl Drop for Stream {
        fn drop(&mut self) {
            let (tx, rx) = mpsc::channel();
            let block = block2::RcBlock::new(move |_err: *mut NSError| {
                let _ = tx.send(());
            });
            unsafe { self.stream.stopCaptureWithCompletionHandler(Some(&block)) };
            let _ = rx.recv_timeout(TIMEOUT);
        }
    }

    pub fn capture_rect(r: Rect, cursor: bool) -> Result<Frame, CaptureError> {
        // macOS 15.2+：直接按屏幕坐标截一块，不用先列出所有显示器和窗口，约 20 ms（旧路径 100~250 ms）。
        // 录屏要画指针，仍走旧路径；系统给的图分辨率不够（比如只给了点而不是像素）也退回旧路径。
        if !cursor && objc2::available!(macos = 15.2) {
            if let Some(s) = scale_at(r) {
                let rect = CGRect::new(
                    CGPoint::new(r.x as f64 / s, r.y as f64 / s),
                    CGSize::new(r.width as f64 / s, r.height as f64 / s),
                );
                match grab(r, |block| unsafe { SCScreenshotManager::captureImageInRect_completionHandler(rect, Some(block)) }) {
                    Err(Grab::LowRes) => {}
                    other => return other.map_err(Grab::into_error),
                }
            }
        }
        capture_with_filter(r, cursor)
    }

    /// 全局点坐标 (x, y) 所在显示器的缩放倍数；不在任何显示器上时取主显示器的。
    /// 窗口吸附（`snap`）把系统给的点坐标换成和截图一致的物理像素时用。
    pub(crate) fn scale_at_point(x: f64, y: f64) -> f64 {
        let mut ids = [0 as CGDirectDisplayID; 16];
        let mut n = 0u32;
        unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut n) };
        let ids = &ids[..n as usize];
        ids.iter()
            .find(|&&id| {
                let b = CGDisplayBounds(id);
                x >= b.origin.x && y >= b.origin.y && x < b.origin.x + b.size.width && y < b.origin.y + b.size.height
            })
            .or(ids.first())
            .map_or(1.0, |&id| scale_of(id))
    }

    /// 物理像素 → 全局点坐标：先找出像素落在哪块显示器（显示器的点范围乘以它自己的倍数），再除以那个倍数。
    pub(crate) fn pixel_to_point(x: i32, y: i32) -> (f64, f64) {
        let mut ids = [0 as CGDirectDisplayID; 16];
        let mut n = 0u32;
        unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut n) };
        let ids = &ids[..n as usize];
        let (fx, fy) = (x as f64, y as f64);
        let s = ids
            .iter()
            .map(|&id| (scale_of(id), CGDisplayBounds(id)))
            .find(|(s, b)| {
                fx >= b.origin.x * s
                    && fy >= b.origin.y * s
                    && fx < (b.origin.x + b.size.width) * s
                    && fy < (b.origin.y + b.size.height) * s
            })
            .map(|(s, _)| s)
            .or_else(|| ids.first().map(|&id| scale_of(id)))
            .unwrap_or(1.0);
        (fx / s, fy / s)
    }

    /// 矩形所在显示器的缩放倍数：矩形左上角换算成点之后落在该显示器里。
    fn scale_at(r: Rect) -> Option<f64> {
        let mut ids = [0 as CGDirectDisplayID; 16];
        let mut n = 0u32;
        unsafe { CGGetActiveDisplayList(ids.len() as u32, ids.as_mut_ptr(), &mut n) };
        ids[..n as usize].iter().find_map(|&id| {
            let s = scale_of(id);
            let b = CGDisplayBounds(id);
            let (px, py) = (r.x as f64 / s, r.y as f64 / s);
            let inside = px >= b.origin.x - 0.5
                && py >= b.origin.y - 0.5
                && px < b.origin.x + b.size.width
                && py < b.origin.y + b.size.height;
            inside.then_some(s)
        })
    }

    enum Grab {
        LowRes,
        Os(String),
        Timeout,
    }

    impl Grab {
        fn into_error(self) -> CaptureError {
            match self {
                Grab::Os(e) => CaptureError::Os(format!("ScreenCaptureKit：{e}")),
                Grab::Timeout => CaptureError::Os("截屏超时".into()),
                Grab::LowRes => CaptureError::Os("截到的图分辨率不够".into()),
            }
        }
    }

    /// 发起一次截图（`start` 把回调交给系统），等系统把图送回来并转成 w×h 的帧。
    fn grab(
        r: Rect,
        start: impl FnOnce(&block2::DynBlock<dyn Fn(*mut CGImage, *mut NSError)>),
    ) -> Result<Frame, Grab> {
        let (tx, rx) = mpsc::channel();
        let (w, h) = (r.width, r.height);
        let block = block2::RcBlock::new(move |image: *mut CGImage, err: *mut NSError| {
            let r = if image.is_null() {
                Err(Grab::Os(unsafe { err.as_ref() }.map(|e| e.localizedDescription().to_string()).unwrap_or_default()))
            } else if unsafe { CGImageGetWidth(image as *const c_void) } < w as usize {
                Err(Grab::LowRes)
            } else {
                Ok(unsafe { to_bgra(image as *const c_void, w, h) })
            };
            let _ = tx.send(r);
        });
        start(&block);
        match rx.recv_timeout(TIMEOUT) {
            Ok(Ok(bgra)) => Ok(Frame::from_bgra(w, h, &bgra)),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(Grab::Timeout),
        }
    }

    /// 旧路径（macOS 14 起可用）：先列出显示器，再按显示器 + 区域截图。
    fn capture_with_filter(r: Rect, cursor: bool) -> Result<Frame, CaptureError> {
        let content = shareable_content()?;
        let displays = unsafe { content.displays() };
        let Some(s) = scale_at(r) else { return Err(CaptureError::Os("找不到截屏区域所在的显示器".into())) };
        let (px, py) = (r.x as f64 / s, r.y as f64 / s);
        let display = displays.iter().find(|d| {
            let f = unsafe { d.frame() };
            px >= f.origin.x - 0.5 && py >= f.origin.y - 0.5 && px < f.origin.x + f.size.width && py < f.origin.y + f.size.height
        });
        let Some(display) = display else { return Err(CaptureError::Os("找不到截屏区域所在的显示器".into())) };
        let f = unsafe { display.frame() };
        let source = CGRect::new(
            CGPoint::new(px - f.origin.x, py - f.origin.y),
            CGSize::new(r.width as f64 / s, r.height as f64 / s),
        );
        let filter = unsafe {
            SCContentFilter::initWithDisplay_excludingWindows(SCContentFilter::alloc(), &display, &NSArray::new())
        };
        let config = unsafe { SCStreamConfiguration::new() };
        unsafe {
            config.setSourceRect(source);
            config.setWidth(r.width as usize);
            config.setHeight(r.height as usize);
            config.setShowsCursor(cursor);
            config.setCaptureResolution(SCCaptureResolutionType::Best);
        }
        grab(r, |block| unsafe {
            SCScreenshotManager::captureImageWithFilter_configuration_completionHandler(&filter, &config, Some(block))
        })
        .map_err(Grab::into_error)
    }

    /// 把 CGImage 画进一块 w×h 的 BGRA 内存（不管系统给的图是什么格式、行宽有没有对齐）。
    /// 用图片自己的色彩空间，像素值和屏幕上一致，不做色彩转换。
    unsafe fn to_bgra(image: *const c_void, w: u32, h: u32) -> Vec<u8> {
        let mut buf = vec![0u8; w as usize * h as usize * 4];
        unsafe {
            let mut space = CGImageGetColorSpace(image);
            let own = space.is_null();
            if own {
                space = CGColorSpaceCreateDeviceRGB();
            }
            let ctx = CGBitmapContextCreate(buf.as_mut_ptr() as *mut c_void, w as usize, h as usize, 8, w as usize * 4, space, BGRA);
            if !ctx.is_null() {
                let rect = CGRect::new(CGPoint::new(0.0, 0.0), CGSize::new(w as f64, h as f64));
                CGContextDrawImage(ctx, rect, image);
                CGContextRelease(ctx);
            }
            if own {
                CGColorSpaceRelease(space);
            }
        }
        // 屏幕内容不透明；预乘 alpha 在 alpha = 255 时就是原值
        for px in buf.chunks_exact_mut(4) {
            px[3] = 255;
        }
        buf
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use super::*;

    /// 每次现抓一帧。
    pub struct Stream(Rect);
    impl Stream {
        pub fn start(r: Rect, _fps: u32) -> Result<Self, CaptureError> {
            Ok(Self(r))
        }
        pub fn frame(&mut self) -> Result<Frame, CaptureError> {
            capture_rect(self.0, true)
        }
    }

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

/// macOS 截屏要「屏幕录制」权限，运行测试的终端未必有，所以默认不跑：`cargo test -- --ignored`。
#[cfg(all(test, target_os = "macos"))]
mod mac_tests {
    use super::*;

    #[test]
    #[ignore]
    fn captures_a_small_region() {
        let r = Rect { x: 0, y: 0, width: 64, height: 32 };
        let t0 = std::time::Instant::now();
        let f = capture_rect(r).expect("截屏应当成功");
        eprintln!("截屏耗时 {:?}", t0.elapsed());
        assert_eq!((f.width, f.height), (64, 32));
        assert!(f.pixels().chunks_exact(4).all(|p| p[3] == 255));
        // 菜单栏不会是一片纯黑（没有权限时系统只给黑图或报错）
        assert!(f.pixels().chunks_exact(4).any(|p| p[0] | p[1] | p[2] != 0));
    }
}

#[cfg(all(test, target_os = "macos"))]
mod stream_tests {
    /// 真机：开 1 秒 30 帧的录屏流，看能不能持续拿到正确尺寸的帧、每帧多快（需要屏幕录制授权、屏幕亮着）。
    #[test]
    #[ignore]
    fn stream_delivers_frames() {
        let r = crate::Rect { x: 0, y: 0, width: 800, height: 600 };
        let t = std::time::Instant::now();
        let mut s = super::Stream::start(r, 30).expect("开不了录屏流");
        eprintln!("启动 {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        let mut slow = std::time::Duration::ZERO;
        for _ in 0..30 {
            let t = std::time::Instant::now();
            let f = s.frame().unwrap();
            slow = slow.max(t.elapsed());
            assert_eq!((f.width, f.height), (800, 600));
            assert!(f.pixels().chunks_exact(4).any(|p| p[0] != 0 || p[1] != 0 || p[2] != 0), "全黑");
            std::thread::sleep(std::time::Duration::from_millis(33));
        }
        eprintln!("30 次取帧，最慢 {:.1} ms", slow.as_secs_f64() * 1e3);
    }
}
