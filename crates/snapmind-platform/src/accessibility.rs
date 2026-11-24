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
