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

#[cfg(windows)]
mod imp {
    use windows::core::BOOL;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, SW_SHOWNOACTIVATE,
        WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

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
}
