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

/// 去掉标题栏 / 边框相关的窗口样式，改成纯弹出窗口。系统会给带标题栏样式的窗口强加一个最小宽度
/// （约 136 像素），悬浮球这种小窗口要先去掉这些样式才能做到 56 像素。系统拖动（startDragging）照样可用。
pub fn make_popup(handle: isize) {
    imp::make_popup(handle)
}

/// 不让截屏抓到这个窗口（长截图时遮罩还在屏幕上，但每一帧都要抓到它下面的真实内容）。
/// Windows 10 2004 起支持；返回是否成功。
pub fn exclude_from_capture(handle: isize, on: bool) -> bool {
    imp::exclude_from_capture(handle, on)
}

/// 遮罩窗口要盖住菜单栏和程序坞（macOS 上普通置顶窗口在菜单栏下面；Windows 的置顶已经盖住任务栏）。
pub fn cover_menu_bar(handle: isize) {
    imp::cover_menu_bar(handle)
}

#[cfg(windows)]
mod imp {
    pub fn cover_menu_bar(_handle: isize) {}

    use windows::core::BOOL;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_TRANSITIONS_FORCEDISABLED};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowDisplayAffinity, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE,
        SW_SHOWNOACTIVATE, WDA_EXCLUDEFROMCAPTURE, WDA_NONE, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };

    pub fn make_popup(handle: isize) {
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowPos, GWL_STYLE, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_CAPTION,
            WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME,
        };
        unsafe {
            let style = GetWindowLongPtrW(hwnd(handle), GWL_STYLE);
            let strip = (WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX).0 as isize;
            SetWindowLongPtrW(hwnd(handle), GWL_STYLE, (style & !strip) | WS_POPUP.0 as isize);
            let _ = SetWindowPos(
                hwnd(handle),
                None,
                0,
                0,
                0,
                0,
                SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }

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

/// macOS：句柄是 NSWindow 指针。AppKit 只能在主线程上调，不在主线程时转给主线程异步执行。
#[cfg(target_os = "macos")]
mod imp {
    use objc2_app_kit::{
        NSScreenSaverWindowLevel, NSWindow, NSWindowAnimationBehavior, NSWindowCollectionBehavior, NSWindowSharingType,
    };
    use objc2_foundation::MainThreadMarker;

    fn on_main(handle: isize, f: impl FnOnce(&NSWindow) + Send + 'static) {
        if handle == 0 {
            return;
        }
        let run = move || {
            // SAFETY: 句柄来自 Tauri 的 ns_window()，窗口由 Tauri 持有，比这次调用活得久
            let w = unsafe { &*(handle as *const NSWindow) };
            f(w)
        };
        if MainThreadMarker::new().is_some() {
            run()
        } else {
            dispatch2::DispatchQueue::main().exec_async(run);
        }
    }

    pub fn disable_transitions(handle: isize) {
        on_main(handle, |w| w.setAnimationBehavior(NSWindowAnimationBehavior::None));
    }

    /// 在每个桌面（Space）和全屏应用上都能出现，不参与 Cmd+` 窗口切换。macOS 没有任务栏；
    /// 程序坞图标由整个应用的激活策略决定（见 src-tauri 的 setup）。
    pub fn make_tool_window(handle: isize) {
        on_main(handle, |w| {
            w.setCollectionBehavior(
                w.collectionBehavior()
                    | NSWindowCollectionBehavior::CanJoinAllSpaces
                    | NSWindowCollectionBehavior::FullScreenAuxiliary
                    | NSWindowCollectionBehavior::IgnoresCycle,
            )
        });
    }

    pub fn show_no_activate(handle: isize) {
        on_main(handle, |w| w.orderFrontRegardless());
    }

    pub fn make_popup(_handle: isize) {}

    pub fn exclude_from_capture(handle: isize, on: bool) -> bool {
        on_main(handle, move |w| w.setSharingType(if on { NSWindowSharingType::None } else { NSWindowSharingType::ReadOnly }));
        true
    }

    pub fn cover_menu_bar(handle: isize) {
        let level = NSScreenSaverWindowLevel;
        on_main(handle, move |w| w.setLevel(level));
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn cover_menu_bar(_handle: isize) {}
    pub fn disable_transitions(_handle: isize) {}
    pub fn make_tool_window(_handle: isize) {}
    pub fn show_no_activate(_handle: isize) {}
    pub fn make_popup(_handle: isize) {}
    pub fn exclude_from_capture(_handle: isize, _on: bool) -> bool {
        false
    }
}
