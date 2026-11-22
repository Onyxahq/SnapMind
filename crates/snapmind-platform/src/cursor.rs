//! 系统鼠标指针和滚轮。

/// 把鼠标指针移动 (dx, dy) 个物理像素（W A S D 逐像素移动光标）。
/// 系统会照常发出鼠标移动消息，所以正在进行的拖动也会跟着精确移动。
pub fn nudge(dx: i32, dy: i32) {
    imp::nudge(dx, dy)
}

/// 鼠标指针的位置（虚拟桌面物理像素）。
pub fn position() -> Option<(i32, i32)> {
    imp::position()
}

/// 给某个顶层窗口里、(x, y) 处最深的子窗口发一次滚轮消息（长截图的自动滚动）。
/// `delta` 以 120 为一格，纵向负数向下、横向正数向右。
///
/// 直接投递消息而不是模拟输入：不用把用户的鼠标指针挪进选区，也不受「滚轮发给焦点窗口还是
/// 指针下的窗口」这个系统设置、以及系统级变暗层吞掉模拟输入的影响。
pub fn post_wheel(top: isize, x: i32, y: i32, delta: i32, horizontal: bool) -> bool {
    imp::post_wheel(top, x, y, delta, horizontal)
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
    use windows::Win32::Graphics::Gdi::ScreenToClient;
    use windows::Win32::UI::WindowsAndMessaging::{
        ChildWindowFromPointEx, GetCursorPos, PostMessageW, SetCursorPos, CWP_SKIPDISABLED, CWP_SKIPINVISIBLE,
        CWP_SKIPTRANSPARENT, WM_MOUSEHWHEEL, WM_MOUSEWHEEL,
    };

    pub fn position() -> Option<(i32, i32)> {
        let mut p = POINT::default();
        unsafe { GetCursorPos(&mut p).ok().map(|_| (p.x, p.y)) }
    }

    pub fn post_wheel(top: isize, x: i32, y: i32, delta: i32, horizontal: bool) -> bool {
        unsafe {
            // 一层层往下找 (x, y) 处的子窗口
            let mut h = HWND(top as *mut _);
            for _ in 0..16 {
                let mut p = POINT { x, y };
                if !ScreenToClient(h, &mut p).as_bool() {
                    break;
                }
                let child = ChildWindowFromPointEx(h, p, CWP_SKIPINVISIBLE | CWP_SKIPDISABLED | CWP_SKIPTRANSPARENT);
                if child.0.is_null() || child == h {
                    break;
                }
                h = child;
            }
            let w = ((delta as i16 as u16 as usize) << 16) as usize;
            let l = (((y as i16 as u16 as u32) << 16) | (x as i16 as u16 as u32)) as isize;
            let msg = if horizontal { WM_MOUSEHWHEEL } else { WM_MOUSEWHEEL };
            PostMessageW(Some(h), msg, WPARAM(w), LPARAM(l)).is_ok()
        }
    }

    pub fn nudge(dx: i32, dy: i32) {
        let mut p = POINT::default();
        unsafe {
            if GetCursorPos(&mut p).is_ok() {
                let _ = SetCursorPos(p.x + dx, p.y + dy);
            }
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn nudge(_dx: i32, _dy: i32) {}
    pub fn position() -> Option<(i32, i32)> {
        None
    }
    pub fn post_wheel(_top: isize, _x: i32, _y: i32, _delta: i32, _horizontal: bool) -> bool {
        false
    }
}
