//! 吸附：找出鼠标下的窗口和控件，供截图时自动选中。
//!
//! - 按下截图键时先拍一张**顶层窗口快照**（按 Z 序，物理像素），与冻结帧一致；
//! - 悬停时用 UI Automation 从鼠标下的顶层窗口往里走，沿着「包含鼠标点」的路径一层层
//!   找子元素，得到从最内层控件到窗口本身的一串矩形（吸附链）。滚轮在这串里切换。
//!
//! 截图时遮罩盖在最上面，不能直接问系统「鼠标下是什么」（会得到遮罩自己），
//! 所以先用快照定位顶层窗口，再对那个窗口做 UI Automation 查询。

use crate::Rect;

/// 顶层窗口快照里的一项。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TopWindow {
    /// 原生窗口句柄（HWND 的整数值）
    pub handle: isize,
    /// 可见外框（不含系统留出的隐形缩放边框），物理像素、虚拟桌面坐标
    pub rect: Rect,
}

pub fn contains(r: &Rect, x: i32, y: i32) -> bool {
    x >= r.x && y >= r.y && x < r.x + r.width as i32 && y < r.y + r.height as i32
}

/// 快照按 Z 序从上到下排列，第一个包含该点的就是鼠标下的窗口。
pub fn window_at(windows: &[TopWindow], x: i32, y: i32) -> Option<&TopWindow> {
    windows.iter().find(|w| contains(&w.rect, x, y))
}

/// 往吸附链里追加一个矩形：空矩形和与上一个相同的矩形跳过（很多控件和父容器一样大）。
pub fn push_unique(chain: &mut Vec<Rect>, r: Rect) {
    if r.width == 0 || r.height == 0 {
        return;
    }
    if chain.last() != Some(&r) {
        chain.push(r);
    }
}

/// 拍顶层窗口快照（Z 序从上到下），排除指定进程（自己的遮罩窗口）。
pub fn top_level_windows(exclude_pid: u32) -> Vec<TopWindow> {
    imp::top_level_windows(exclude_pid)
}

/// UI Automation 查询器。内部有一个专用线程（COM 多线程套间），查询是阻塞调用，
/// 不要在 UI 主线程上调用。
pub struct ElementProbe {
    tx: std::sync::mpsc::Sender<Request>,
}

struct Request {
    window: TopWindow,
    x: i32,
    y: i32,
    budget: std::time::Duration,
    reply: std::sync::mpsc::Sender<Vec<Rect>>,
}

impl ElementProbe {
    pub fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel::<Request>();
        std::thread::Builder::new()
            .name("snapmind-uia".into())
            .spawn(move || imp::serve(rx))
            .expect("创建 UI Automation 线程失败");
        Self { tx }
    }

    /// 吸附链：从最内层元素到窗口本身（含窗口），在 `budget` 时间内尽量往深处找。
    /// UI Automation 不可用时只返回窗口矩形。
    pub fn chain(&self, window: TopWindow, x: i32, y: i32, budget: std::time::Duration) -> Vec<Rect> {
        let (reply, rx) = std::sync::mpsc::channel();
        if self.tx.send(Request { window, x, y, budget, reply }).is_err() {
            return vec![window.rect];
        }
        rx.recv().unwrap_or_else(|_| vec![window.rect])
    }
}

impl Default for ElementProbe {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::time::Instant;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Dwm::{
        DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
    };
    use windows::Win32::UI::Accessibility::{
        CUIAutomation, IUIAutomation, IUIAutomationCacheRequest, IUIAutomationElement,
        IUIAutomationTreeWalker, UIA_BoundingRectanglePropertyId, UIA_IsOffscreenPropertyId,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetClassNameW, GetWindowLongW, GetWindowRect, GetWindowThreadProcessId,
        IsIconic, IsWindowVisible, GWL_EXSTYLE, WS_EX_TRANSPARENT,
    };

    /// 不参与吸附的系统窗口类：全屏的系统变暗层等。
    const SKIP_CLASSES: &[&str] = &["Shell_SystemDim"];
    /// 每层最多看多少个子元素、最多走多少层，防止遇到超大列表卡住。
    const MAX_SIBLINGS: usize = 400;
    const MAX_DEPTH: usize = 24;
    /// 每层最多尝试几个候选子元素（回溯的上限）。
    const MAX_TRIES: usize = 3;

    fn to_rect(r: RECT) -> Rect {
        Rect {
            x: r.left,
            y: r.top,
            width: (r.right - r.left).max(0) as u32,
            height: (r.bottom - r.top).max(0) as u32,
        }
    }

    fn class_name(h: HWND) -> String {
        let mut buf = [0u16; 128];
        let n = unsafe { GetClassNameW(h, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    struct EnumCtx {
        exclude_pid: u32,
        out: Vec<TopWindow>,
    }

    unsafe extern "system" fn on_window(h: HWND, lp: LPARAM) -> BOOL {
        let ctx = unsafe { &mut *(lp.0 as *mut EnumCtx) };
        unsafe {
            if !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
                return true.into();
            }
            let mut pid = 0u32;
            GetWindowThreadProcessId(h, Some(&mut pid));
            if pid == ctx.exclude_pid {
                return true.into();
            }
            if GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TRANSPARENT.0 != 0 {
                return true.into(); // 鼠标穿透的悬浮层
            }
            let mut cloaked = 0u32;
            let _ = DwmGetWindowAttribute(
                h,
                DWMWA_CLOAKED,
                &mut cloaked as *mut u32 as *mut _,
                std::mem::size_of::<u32>() as u32,
            );
            if cloaked != 0 {
                return true.into(); // 其他虚拟桌面上的、UWP 挂起的窗口
            }
            if SKIP_CLASSES.contains(&class_name(h).as_str()) {
                return true.into();
            }
            let mut r = RECT::default();
            let ok = DwmGetWindowAttribute(
                h,
                DWMWA_EXTENDED_FRAME_BOUNDS,
                &mut r as *mut RECT as *mut _,
                std::mem::size_of::<RECT>() as u32,
            )
            .is_ok()
                || GetWindowRect(h, &mut r).is_ok();
            let rect = to_rect(r);
            if ok && rect.width > 0 && rect.height > 0 {
                ctx.out.push(TopWindow { handle: h.0 as isize, rect });
            }
        }
        true.into()
    }

    pub fn top_level_windows(exclude_pid: u32) -> Vec<TopWindow> {
        let mut ctx = EnumCtx { exclude_pid, out: Vec::new() };
        unsafe {
            let _ = EnumWindows(Some(on_window), LPARAM(&mut ctx as *mut EnumCtx as isize));
        }
        ctx.out
    }

    struct Uia {
        automation: IUIAutomation,
        walker: IUIAutomationTreeWalker,
        cache: IUIAutomationCacheRequest,
    }

    impl Uia {
        fn new() -> windows::core::Result<Self> {
            unsafe {
                let automation: IUIAutomation = CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER)?;
                let walker = automation.ControlViewWalker()?;
                // 一次跨进程调用就把外框和是否在屏幕外一起取回来
                let cache = automation.CreateCacheRequest()?;
                cache.AddProperty(UIA_BoundingRectanglePropertyId)?;
                cache.AddProperty(UIA_IsOffscreenPropertyId)?;
                Ok(Self { automation, walker, cache })
            }
        }

        fn chain(&self, req: &Request) -> Vec<Rect> {
            let deadline = Instant::now() + req.budget;
            let root = unsafe { self.automation.ElementFromHandle(HWND(req.window.handle as *mut _)) };
            // path 从外到内
            let path = match root {
                Ok(el) => self.descend(&el, req.x, req.y, deadline, 0),
                Err(_) => Vec::new(),
            };
            // 吸附链从内到外，最后是窗口本身；与窗口一样大的层没有意义，去掉
            let mut chain: Vec<Rect> = Vec::with_capacity(path.len() + 1);
            for r in path.into_iter().rev() {
                if r != req.window.rect {
                    super::push_unique(&mut chain, r);
                }
            }
            super::push_unique(&mut chain, req.window.rect);
            chain
        }

        /// 深度优先往里走：子元素里包含该点的按面积从小到大逐个尝试，
        /// 某个候选下面再也找不到子元素（例如和内容区一样大的空渲染层）就换下一个。
        /// 返回从外到内的路径（不含 parent 本身）。
        fn descend(
            &self,
            parent: &IUIAutomationElement,
            x: i32,
            y: i32,
            deadline: Instant,
            depth: usize,
        ) -> Vec<Rect> {
            if depth >= MAX_DEPTH || Instant::now() > deadline {
                return Vec::new();
            }
            let candidates = self.children_at(parent, x, y, deadline);
            let mut leaf: Option<Rect> = None;
            for (child, rect) in candidates.into_iter().take(MAX_TRIES) {
                let deeper = self.descend(&child, x, y, deadline, depth + 1);
                if !deeper.is_empty() {
                    let mut path = vec![rect];
                    path.extend(deeper);
                    return path;
                }
                leaf.get_or_insert(rect);
                if Instant::now() > deadline {
                    break;
                }
            }
            leaf.map(|r| vec![r]).unwrap_or_default()
        }

        /// 包含该点、在屏幕上、面积非零的子元素，按面积从小到大。
        fn children_at(
            &self,
            parent: &IUIAutomationElement,
            x: i32,
            y: i32,
            deadline: Instant,
        ) -> Vec<(IUIAutomationElement, Rect)> {
            let mut found: Vec<(IUIAutomationElement, Rect)> = Vec::new();
            let mut child = unsafe { self.walker.GetFirstChildElementBuildCache(parent, &self.cache) }.ok();
            let mut n = 0;
            while let Some(c) = child {
                n += 1;
                let rect = unsafe { c.CachedBoundingRectangle() }.map(to_rect).ok();
                let offscreen = unsafe { c.CachedIsOffscreen() }.map(|b| b.as_bool()).unwrap_or(false);
                if let Some(r) = rect {
                    if !offscreen && r.width > 0 && r.height > 0 && super::contains(&r, x, y) {
                        found.push((c.clone(), r));
                    }
                }
                if n >= MAX_SIBLINGS || Instant::now() > deadline {
                    break;
                }
                child = unsafe { self.walker.GetNextSiblingElementBuildCache(&c, &self.cache) }.ok();
            }
            // 稳定排序：面积相同时保持原来的顺序
            found.sort_by_key(|(_, r)| r.width as u64 * r.height as u64);
            found
        }
    }

    pub fn serve(rx: std::sync::mpsc::Receiver<Request>) {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let uia = Uia::new().ok();
        while let Ok(mut req) = rx.recv() {
            // 只处理最新的请求：鼠标移动很快，旧位置的结果没有意义
            while let Ok(newer) = rx.try_recv() {
                let _ = req.reply.send(vec![req.window.rect]);
                req = newer;
            }
            let chain = match &uia {
                Some(u) => u.chain(&req),
                None => vec![req.window.rect],
            };
            let _ = req.reply.send(chain);
        }
    }
}

/// macOS：顶层窗口用 `CGWindowListCopyWindowInfo` 拍快照（只读位置和大小，不需要任何权限）。
/// 往窗口里面找控件用辅助功能（AX）接口：让目标程序自己做命中测试（遮罩不在它的进程里，不会挡住），
/// 再沿父元素一路走到窗口。没有辅助功能授权时 AX 调用直接失败，吸附链只有窗口本身。
#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_core_graphics::{kCGNullWindowID, CGWindowListCopyWindowInfo, CGWindowListOption};
    use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString};

    /// 普通应用窗口所在的层（kCGNormalWindowLevel）。菜单栏、程序坞、浮动面板在更高的层，不吸附。
    const NORMAL_LAYER: i64 = 0;

    fn num(d: &NSDictionary<NSString, AnyObject>, key: &str) -> Option<f64> {
        let v = d.objectForKey(&NSString::from_str(key))?;
        v.downcast::<NSNumber>().ok().map(|n| n.doubleValue())
    }

    pub fn top_level_windows(exclude_pid: u32) -> Vec<TopWindow> {
        let opts = CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements;
        let Some(list) = CGWindowListCopyWindowInfo(opts, kCGNullWindowID) else { return Vec::new() };
        // CFArray 和 NSArray 可以互相直接当作对方使用（toll-free bridging）
        // SAFETY: 系统保证数组里每一项都是 CFDictionary（键为 CFString）。
        // SAFETY: 同上；`list` 活到函数结束，借用不会悬空。
        let list: &NSArray<NSDictionary<NSString, AnyObject>> =
            unsafe { &*(&*list as *const objc2_core_foundation::CFArray).cast() };
        let mut out = Vec::new();
        // 系统按前后顺序给出，最前面的在第一个，和 Windows 的 Z 序一致
        for d in list.iter() {
            if num(&d, "kCGWindowLayer") != Some(NORMAL_LAYER as f64) {
                continue;
            }
            if num(&d, "kCGWindowOwnerPID").is_some_and(|p| p as u32 == exclude_pid) {
                continue;
            }
            if num(&d, "kCGWindowAlpha").is_some_and(|a| a <= 0.0) {
                continue;
            }
            let Some(b) = d.objectForKey(&NSString::from_str("kCGWindowBounds")) else { continue };
            let Ok(b) = b.downcast::<NSDictionary>() else { continue };
            // SAFETY: 窗口外框字典的键都是字符串
            let b: Retained<NSDictionary<NSString, AnyObject>> = unsafe { Retained::cast_unchecked(b) };
            let (Some(x), Some(y), Some(w), Some(h)) = (num(&b, "X"), num(&b, "Y"), num(&b, "Width"), num(&b, "Height")) else {
                continue;
            };
            let rect = to_pixels(x, y, w, h, crate::capture::scale_at_point(x, y));
            if rect.width == 0 || rect.height == 0 {
                continue;
            }
            let handle = num(&d, "kCGWindowNumber").unwrap_or(0.0) as isize;
            out.push(TopWindow { handle, rect });
        }
        out
    }

    /// 窗口（kCGWindowNumber）属于哪个进程。长截图往指定窗口发滚轮时用。
    pub(crate) fn window_owner_pid(handle: isize) -> Option<i32> {
        let list = CGWindowListCopyWindowInfo(CGWindowListOption::OptionIncludingWindow, handle as u32)?;
        // SAFETY: 同 top_level_windows
        let list: &NSArray<NSDictionary<NSString, AnyObject>> =
            unsafe { &*(&*list as *const objc2_core_foundation::CFArray).cast() };
        let d = list.iter().next()?;
        num(&d, "kCGWindowOwnerPID").map(|p| p as i32)
    }

    /// 点坐标 → 物理像素（tao 约定：乘以窗口所在显示器的缩放倍数）。
    pub(super) fn to_pixels(x: f64, y: f64, w: f64, h: f64, scale: f64) -> Rect {
        let (x0, y0) = ((x * scale).round() as i32, (y * scale).round() as i32);
        let (x1, y1) = (((x + w) * scale).round() as i32, ((y + h) * scale).round() as i32);
        Rect { x: x0, y: y0, width: (x1 - x0).max(0) as u32, height: (y1 - y0).max(0) as u32 }
    }

    use crate::accessibility::ax;

    /// 最多往上走这么多层（正常界面十几层就到窗口了）。
    const MAX_DEPTH: usize = 40;

    fn intersect(a: Rect, b: Rect) -> Rect {
        let (x0, y0) = (a.x.max(b.x), a.y.max(b.y));
        let x1 = (a.x + a.width as i32).min(b.x + b.width as i32);
        let y1 = (a.y + a.height as i32).min(b.y + b.height as i32);
        Rect { x: x0, y: y0, width: (x1 - x0).max(0) as u32, height: (y1 - y0).max(0) as u32 }
    }

    fn chain(req: &Request) -> Vec<Rect> {
        let deadline = std::time::Instant::now() + req.budget;
        let mut chain = Vec::new();
        // 任何一步失败（没授权、程序不支持辅助功能、超时）就停在已找到的层
        let _ = (|| {
            let pid = window_owner_pid(req.window.handle)?;
            let app = ax::app(pid, req.budget.as_secs_f32().max(0.05))?;
            let (px, py) = crate::capture::pixel_to_point(req.x, req.y);
            let scale = crate::capture::scale_at_point(px, py);
            let mut el = ax::element_at(&app, px, py)?;
            for _ in 0..MAX_DEPTH {
                if let Some((x, y, w, h)) = ax::frame(&el) {
                    // 滚动区域里的内容可能比窗口还大，只取窗口里看得见的部分；不含鼠标的层跳过
                    let r = intersect(to_pixels(x, y, w, h, scale), req.window.rect);
                    if r != req.window.rect && contains(&r, req.x, req.y) {
                        push_unique(&mut chain, r);
                    }
                }
                if ax::role(&el) == "AXWindow" || std::time::Instant::now() > deadline {
                    break;
                }
                el = ax::parent(&el)?;
            }
            Some(())
        })();
        push_unique(&mut chain, req.window.rect);
        chain
    }

    pub fn serve(rx: std::sync::mpsc::Receiver<Request>) {
        while let Ok(mut req) = rx.recv() {
            // 只处理最新的请求：鼠标移动很快，旧位置的结果没有意义
            while let Ok(newer) = rx.try_recv() {
                let _ = req.reply.send(vec![req.window.rect]);
                req = newer;
            }
            let _ = req.reply.send(chain(&req));
        }
    }
}

#[cfg(target_os = "macos")]
pub(crate) use imp::window_owner_pid;

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use super::*;

    pub fn top_level_windows(_exclude_pid: u32) -> Vec<TopWindow> {
        Vec::new()
    }

    pub fn serve(rx: std::sync::mpsc::Receiver<Request>) {
        while let Ok(req) = rx.recv() {
            let _ = req.reply.send(vec![req.window.rect]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, width: w, height: h }
    }

    #[test]
    fn topmost_window_wins() {
        let ws = [
            TopWindow { handle: 1, rect: r(100, 100, 200, 200) },
            TopWindow { handle: 2, rect: r(0, 0, 1000, 1000) },
        ];
        assert_eq!(window_at(&ws, 150, 150).unwrap().handle, 1);
        assert_eq!(window_at(&ws, 50, 50).unwrap().handle, 2);
        assert!(window_at(&ws, 2000, 0).is_none());
        // 右边和下边不算在内
        assert_eq!(window_at(&ws, 300, 150).unwrap().handle, 2);
    }

    #[test]
    fn chain_skips_empty_and_duplicates() {
        let mut c = Vec::new();
        push_unique(&mut c, r(0, 0, 10, 10));
        push_unique(&mut c, r(0, 0, 10, 10));
        push_unique(&mut c, r(5, 5, 0, 3));
        push_unique(&mut c, r(0, 0, 20, 20));
        assert_eq!(c, vec![r(0, 0, 10, 10), r(0, 0, 20, 20)]);
    }

    #[cfg(windows)]
    #[test]
    fn snapshot_lists_some_windows() {
        // 交互式桌面上至少有任务栏
        let ws = top_level_windows(std::process::id());
        assert!(!ws.is_empty());
        assert!(ws.iter().all(|w| w.rect.width > 0 && w.rect.height > 0));
    }

    #[cfg(windows)]
    #[test]
    fn probe_returns_window_as_outermost() {
        let ws = top_level_windows(std::process::id());
        let w = ws[0];
        let probe = ElementProbe::new();
        let chain = probe.chain(w, w.rect.x + 5, w.rect.y + 5, std::time::Duration::from_millis(300));
        assert_eq!(chain.last(), Some(&w.rect));
    }
}

/// 手动诊断：`cargo test -p snapmind-platform -- --ignored --nocapture probe_report`
/// 打印屏幕上几个点的吸附链和耗时。
#[cfg(all(test, target_os = "macos"))]
mod mac_tests {
    use super::*;

    #[test]
    fn points_become_pixels_of_that_display() {
        // Retina（2 倍）上 (10.25, 20) 起 100×50 点的窗口
        assert_eq!(imp::to_pixels(10.25, 20.0, 100.0, 50.0, 2.0), Rect { x: 21, y: 40, width: 200, height: 100 });
        // 主显示器左边的外接 1 倍屏，坐标是负的
        assert_eq!(imp::to_pixels(-1920.0, 0.0, 800.0, 600.0, 1.0), Rect { x: -1920, y: 0, width: 800, height: 600 });
    }

    /// 真机：在每个窗口中央问吸附链（需要运行测试的终端有辅助功能授权）。
    #[test]
    #[ignore]
    fn element_chain_inside_real_windows() {
        let probe = ElementProbe::new();
        let mut deeper = 0;
        for w in top_level_windows(std::process::id()).into_iter().take(6) {
            let (x, y) = (w.rect.x + w.rect.width as i32 / 2, w.rect.y + w.rect.height as i32 / 2);
            let t = std::time::Instant::now();
            let chain = probe.chain(w, x, y, std::time::Duration::from_millis(150));
            eprintln!("窗口 {:?}：{} 层，{:.1} ms，{:?}", w.rect, chain.len(), t.elapsed().as_secs_f64() * 1e3, chain);
            assert_eq!(chain.last(), Some(&w.rect), "最后一层是窗口本身");
            assert!(chain.iter().all(|r| contains(r, x, y)), "每层都包含鼠标点");
            deeper += (chain.len() > 1) as usize;
        }
        assert!(deeper > 0, "至少有一个窗口能找到控件");
    }

    #[test]
    fn lists_other_apps_windows_front_to_back() {
        let ws = top_level_windows(std::process::id());
        eprintln!("{} 个窗口：{:?}", ws.len(), ws.iter().take(5).collect::<Vec<_>>());
        assert!(ws.iter().all(|w| w.rect.width > 0 && w.rect.height > 0));
    }
}

#[cfg(all(test, windows))]
#[test]
#[ignore]
fn probe_report() {
    let ws = top_level_windows(std::process::id());
    let probe = ElementProbe::new();
    for (x, y) in [(400, 300), (2000, 400), (1280, 720), (60, 40), (1280, 1420)] {
        let Some(w) = window_at(&ws, x, y) else { continue };
        let t = std::time::Instant::now();
        let chain = probe.chain(*w, x, y, std::time::Duration::from_millis(150));
        println!("({x},{y}) {:.1} ms, {} 层: {:?}", t.elapsed().as_secs_f64() * 1e3, chain.len(), chain.iter().map(|r| (r.width, r.height)).collect::<Vec<_>>());
    }
}
