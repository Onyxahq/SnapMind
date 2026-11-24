//! 辅助功能权限：截图时往窗口里找控件、长截图自动滚动、划词取选中文字要用到。
//! 只有 macOS 需要用户授权（系统设置 → 隐私与安全性 → 辅助功能），其他平台总是有。

/// 有没有辅助功能权限。
pub fn has_permission() -> bool {
    imp::has_permission()
}

/// 申请辅助功能权限：没授权时系统弹出提示，提示里可以直接去系统设置打开开关（授权后立即生效，不用重启）。
/// `open_settings` 为 true 时再直接打开「辅助功能」设置页（用户已经关掉过提示的情况）。
pub fn request_permission(open_settings: bool) {
    imp::request_permission(open_settings)
}

/// 辅助功能（AX）接口的最小封装：snap（控件吸附）和 selection（划词）共用。
#[cfg(target_os = "macos")]
pub(crate) mod ax {
    use std::ffi::c_void;
    use objc2_core_foundation::CFString;

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> *const c_void;
        fn AXUIElementCopyElementAtPosition(app: *const c_void, x: f32, y: f32, out: *mut *const c_void) -> i32;
        fn AXUIElementCopyAttributeValue(el: *const c_void, attr: *const c_void, out: *mut *const c_void) -> i32;
        fn AXUIElementSetMessagingTimeout(el: *const c_void, seconds: f32) -> i32;
        fn AXValueGetValue(value: *const c_void, kind: u32, out: *mut c_void) -> u8;
        fn AXUIElementSetAttributeValue(el: *const c_void, attr: *const c_void, value: *const c_void) -> i32;
        fn AXUIElementCreateSystemWide() -> *const c_void;
    }
    #[link(name = "CoreFoundation", kind = "framework")]
    unsafe extern "C" {
        fn CFRelease(cf: *const c_void);
        fn CFGetTypeID(cf: *const c_void) -> usize;
        fn CFStringGetTypeID() -> usize;
        static kCFBooleanTrue: *const c_void;
    }
    const AX_VALUE_CGPOINT: u32 = 1;
    const AX_VALUE_CGSIZE: u32 = 2;

    /// 自己持有的一个 CF 对象，离开作用域时释放。
    pub struct Owned(*const c_void);
    impl Drop for Owned {
        fn drop(&mut self) {
            // SAFETY: 只装 Copy / Create 系列函数返回的对象，引用归我们所有
            unsafe { CFRelease(self.0) }
        }
    }
    // SAFETY: AX 元素是不可变的引用计数对象，可以在线程间移动
    unsafe impl Send for Owned {}

    fn attr(el: &Owned, name: &'static str) -> Option<Owned> {
        let key = CFString::from_static_str(name);
        let mut out = std::ptr::null();
        // SAFETY: el 有效；成功时 out 是一个新的 +1 引用
        let err = unsafe { AXUIElementCopyAttributeValue(el.0, &*key as *const CFString as *const c_void, &mut out) };
        (err == 0 && !out.is_null()).then(|| Owned(out))
    }

    pub fn app(pid: i32, timeout_secs: f32) -> Option<Owned> {
        // SAFETY: 任意 pid 都可以传，不存在时后续调用返回错误
        let a = unsafe { AXUIElementCreateApplication(pid) };
        if a.is_null() {
            return None;
        }
        let a = Owned(a);
        prepare_app(&a, timeout_secs);
        Some(a)
    }

    /// 设好跨进程调用的超时；Chromium / Electron 系（Chrome、VS Code 等）默认不对外提供完整的控件树，
    /// 要有人把 AXManualAccessibility 设成 true 才打开——不认这个属性的程序返回错误，没有影响。
    fn prepare_app(a: &Owned, timeout_secs: f32) {
        let key = CFString::from_static_str("AXManualAccessibility");
        // SAFETY: a 有效；kCFBooleanTrue 是系统常量
        unsafe {
            AXUIElementSetMessagingTimeout(a.0, timeout_secs);
            AXUIElementSetAttributeValue(a.0, &*key as *const CFString as *const c_void, kCFBooleanTrue);
        }
    }

    /// 当前有键盘焦点的程序（已设好超时并打开 Chromium 的控件树）。
    pub fn focused_app(timeout_secs: f32) -> Option<Owned> {
        // SAFETY: 无参数，返回一个 +1 引用
        let sys = Owned(unsafe { AXUIElementCreateSystemWide() });
        unsafe { AXUIElementSetMessagingTimeout(sys.0, timeout_secs) };
        let app = attr(&sys, "AXFocusedApplication")?;
        prepare_app(&app, timeout_secs);
        Some(app)
    }

    /// 程序里有键盘焦点的控件。
    pub fn focused_element(app: &Owned) -> Option<Owned> {
        attr(app, "AXFocusedUIElement")
    }

    /// 字符串属性。控件不支持这个属性（或值不是字符串）时为 None；支持但为空时是 Some("")。
    pub fn string_attr(el: &Owned, name: &'static str) -> Option<String> {
        let v = attr(el, name)?;
        // SAFETY: 先核对类型再按 CFString 读
        unsafe { (CFGetTypeID(v.0) == CFStringGetTypeID()).then(|| (*(v.0 as *const CFString)).to_string()) }
    }

    pub fn element_at(app: &Owned, x: f64, y: f64) -> Option<Owned> {
        let mut out = std::ptr::null();
        let err = unsafe { AXUIElementCopyElementAtPosition(app.0, x as f32, y as f32, &mut out) };
        (err == 0 && !out.is_null()).then(|| Owned(out))
    }

    pub fn parent(el: &Owned) -> Option<Owned> {
        attr(el, "AXParent")
    }

    pub fn role(el: &Owned) -> String {
        string_attr(el, "AXRole").unwrap_or_default()
    }

    /// 元素外框（全局点坐标）：(x, y, 宽, 高)。
    pub fn frame(el: &Owned) -> Option<(f64, f64, f64, f64)> {
        let pos = attr(el, "AXPosition")?;
        let size = attr(el, "AXSize")?;
        let mut p = [0f64; 2];
        let mut s = [0f64; 2];
        // SAFETY: AXPosition / AXSize 是 AXValue，里面分别是 CGPoint / CGSize（两个 f64）
        let ok = unsafe {
            AXValueGetValue(pos.0, AX_VALUE_CGPOINT, p.as_mut_ptr() as *mut c_void) != 0
                && AXValueGetValue(size.0, AX_VALUE_CGSIZE, s.as_mut_ptr() as *mut c_void) != 0
        };
        ok.then_some((p[0], p[1], s[0], s[1]))
    }
}

#[cfg(windows)]
mod imp {
    pub fn has_permission() -> bool {
        true
    }
    pub fn request_permission(_open_settings: bool) {}
}

#[cfg(target_os = "macos")]
mod imp {
    use objc2_core_foundation::{CFBoolean, CFDictionary, CFString};

    #[link(name = "ApplicationServices", kind = "framework")]
    unsafe extern "C" {
        fn AXIsProcessTrusted() -> u8;
        fn AXIsProcessTrustedWithOptions(options: *const CFDictionary) -> u8;
    }

    pub fn has_permission() -> bool {
        // SAFETY: 无参数，只读当前进程的授权状态
        unsafe { AXIsProcessTrusted() != 0 }
    }

    pub fn request_permission(open_settings: bool) {
        // kAXTrustedCheckOptionPrompt 的值就是这个字符串
        let key = CFString::from_static_str("AXTrustedCheckOptionPrompt");
        let yes = CFBoolean::new(true);
        let options = CFDictionary::from_slices(&[&*key], &[yes]);
        // SAFETY: options 是合法的 CFDictionary，调用期间一直有效
        let trusted = unsafe { AXIsProcessTrustedWithOptions(&*options as *const _ as *const CFDictionary) } != 0;
        if !trusted && open_settings {
            let _ = std::process::Command::new("open")
                .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
                .spawn();
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn has_permission() -> bool {
        true
    }
    pub fn request_permission(_open_settings: bool) {}
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_without_panicking() {
        let _ = super::has_permission();
    }
}
