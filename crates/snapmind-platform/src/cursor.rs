//! 系统鼠标指针。

/// 把鼠标指针移动 (dx, dy) 个物理像素（W A S D 逐像素移动光标）。
/// 系统会照常发出鼠标移动消息，所以正在进行的拖动也会跟着精确移动。
pub fn nudge(dx: i32, dy: i32) {
    imp::nudge(dx, dy)
}

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, SetCursorPos};

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
}
