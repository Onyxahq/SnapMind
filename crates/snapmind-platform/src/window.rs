//! 原生窗口的小调整。参数是原生窗口句柄（Windows 上是 HWND 的整数值）。

/// 关闭窗口显示 / 隐藏、移动时的系统过渡动画。截图遮罩要求按下快捷键立刻出现，
/// 系统默认的淡入会拖慢约 100–200 ms，也显得不跟手。
pub fn disable_transitions(handle: isize) {
    imp::disable_transitions(handle)
}

/// 设为工具窗口：不出现在任务栏和 Alt+Tab 里。
pub fn make_tool_window(handle: isize) {
    imp::make_tool_window(handle)
}

/// 显示窗口但不激活（不抢走用户当前窗口的焦点）。
pub fn show_no_activate(handle: isize) {
    imp::show_no_activate(handle)
}

/// 不让截屏抓到这个窗口（长截图时遮罩还在屏幕上，但每一帧都要抓到它下面的真实内容）。
/// Windows 10 2004 起支持；返回是否成功。
pub fn exclude_from_capture(handle: isize, on: bool) -> bool {
    imp::exclude_from_capture(handle, on)
}

#[cfg(windows)]
mod imp {
    use windows::core::BOOL;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowDisplayAffinity, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE,
        SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    pub fn exclude_from_capture(handle: isize, on: bool) -> bool {
        unsafe { SetWindowDisplayAffinity(hwnd(handle), if on { WDA_EXCLUDEFROMCAPTURE } else { WDA_NONE }).is_ok() }
    }

    fn hwnd(handle: isize) -> HWND {
        HWND(handle as *mut _)
    }

    pub fn disable_transitions(handle: isize) {
        let on = BOOL::from(true);
        unsafe {
            let _ = DwmSetWindowAttribute(
                hwnd(handle),
                DWMWA_TRANSITIONS_FORCEDISABLED,
                &on as *const BOOL as *const _,
                std::mem::size_of::<BOOL>() as u32,
            );
        }
    }

    pub fn make_tool_window(handle: isize) {
        unsafe {
            let ex = GetWindowLongPtrW(hwnd(handle), GWL_EXSTYLE);
            let ex = (ex | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
            SetWindowLongPtrW(hwnd(handle), GWL_EXSTYLE, ex);
        }
    }

    pub fn show_no_activate(handle: isize) {
        unsafe {
            let _ = ShowWindow(hwnd(handle), SW_SHOWNOACTIVATE);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn disable_transitions(_handle: isize) {}
    pub fn make_tool_window(_handle: isize) {}
    pub fn show_no_activate(_handle: isize) {}
    pub fn exclude_from_capture(_handle: isize, _on: bool) -> bool {
        false
    }
}
