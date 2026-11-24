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

/// 鼠标左键现在是否按着（悬浮球判断拖动是否结束）。
pub fn left_button_down() -> bool {
    imp::left_button_down()
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

    pub fn left_button_down() -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
        unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
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

/// macOS：读位置 / 按键状态不需要权限；移动指针、发滚轮要「辅助功能」授权，
/// 没授权时移动指针退回 `CGWarpMouseCursorPosition`（不会带动正在进行的拖动），滚轮发不出去。
#[cfg(target_os = "macos")]
mod imp {
    use crate::capture::{pixel_to_point, scale_at_point};

    use objc2_core_foundation::CGPoint;
    use objc2_core_graphics::{
        CGEvent, CGEventField, CGEventSource, CGEventSourceStateID, CGEventTapLocation, CGEventType, CGMouseButton,
        CGScrollEventUnit, CGWarpMouseCursorPosition,
    };

    use crate::accessibility::has_permission as trusted;

    fn location() -> Option<CGPoint> {
        let e = CGEvent::new(None)?;
        Some(CGEvent::location(Some(&e)))
    }

    pub fn position() -> Option<(i32, i32)> {
        let p = location()?;
        let s = scale_at_point(p.x, p.y);
        Some(((p.x * s).round() as i32, (p.y * s).round() as i32))
    }

    pub fn left_button_down() -> bool {
        CGEventSource::button_state(CGEventSourceStateID::CombinedSessionState, CGMouseButton::Left)
    }

    /// 指针只能停在整数点上（Retina 上 1 点 = 2 像素），所以每次至少挪 1 点，不然 1 像素的微移会被吞掉。
    pub(super) fn whole_points(d: i32, scale: f64) -> f64 {
        if d == 0 { 0.0 } else { (d as f64 / scale).abs().round().max(1.0).copysign(d as f64) }
    }

    pub fn nudge(dx: i32, dy: i32) {
        let Some(p) = location() else { return };
        let s = scale_at_point(p.x, p.y);
        let to = CGPoint::new(p.x.round() + whole_points(dx, s), p.y.round() + whole_points(dy, s));
        if trusted() {
            // 发一个真实的移动（按着左键时是拖动）事件：指针跟着动，正在进行的拖动也会精确跟上
            let kind = if left_button_down() { CGEventType::LeftMouseDragged } else { CGEventType::MouseMoved };
            if let Some(e) = CGEvent::new_mouse_event(None, kind, to, CGMouseButton::Left) {
                CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&e));
                return;
            }
        }
        let _ = CGWarpMouseCursorPosition(to);
    }

    /// 往窗口 `top`（kCGWindowNumber）所属进程直接投递滚轮事件，并把事件标成「鼠标下是这个窗口」：
    /// 遮罩盖在最上面，发到系统事件流里会被遮罩自己收到。
    /// 用像素单位：120 = Windows 的一格 ≈ 60 点，40（小选区的三分之一格）≈ 20 点。
    pub fn post_wheel(top: isize, x: i32, y: i32, delta: i32, horizontal: bool) -> bool {
        if !trusted() {
            return false;
        }
        let Some(pid) = crate::snap::window_owner_pid(top) else { return false };
        let amount = delta / 2;
        // 纵向：两边都是正数向上。横向：Windows 正数向右，macOS 的第二个滚轮正数向左（这是推断，待真机核对）
        let (w1, w2) = if horizontal { (0, -amount) } else { (amount, 0) };
        let Some(e) = CGEvent::new_scroll_wheel_event2(None, CGScrollEventUnit::Pixel, 2, w1, w2, 0) else {
            return false;
        };
        let (px, py) = pixel_to_point(x, y);
        CGEvent::set_location(Some(&e), CGPoint::new(px, py));
        for field in [
            CGEventField::MouseEventWindowUnderMousePointer,
            CGEventField::MouseEventWindowUnderMousePointerThatCanHandleThisEvent,
        ] {
            CGEvent::set_integer_value_field(Some(&e), field, top as i64);
        }
        CGEvent::post_to_pid(pid, Some(&e));
        true
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    pub fn nudge(_dx: i32, _dy: i32) {}
    pub fn position() -> Option<(i32, i32)> {
        None
    }
    pub fn left_button_down() -> bool {
        false
    }
    pub fn post_wheel(_top: isize, _x: i32, _y: i32, _delta: i32, _horizontal: bool) -> bool {
        false
    }
}

#[cfg(all(test, target_os = "macos"))]
mod mac_tests {
    #[test]
    fn nudge_moves_at_least_one_point() {
        let w = super::imp::whole_points;
        assert_eq!(w(1, 2.0), 1.0); // Retina 上 1 像素也挪 1 点
        assert_eq!(w(-1, 2.0), -1.0);
        assert_eq!(w(3, 2.0), 2.0);
        assert_eq!(w(4, 1.0), 4.0);
        assert_eq!(w(0, 2.0), 0.0);
    }

    /// 真机：读位置、左右各挪 3 像素、往一个真实窗口投递 0 距离滚轮（不会真的滚动别人的内容）。
    /// 需要运行测试的终端有辅助功能授权；屏幕锁定时位置读得到，但挪动可能不生效。
    #[test]
    #[ignore]
    fn moves_pointer_and_posts_wheel() {
        let (x, y) = super::position().expect("读不到指针位置");
        super::nudge(3, 0);
        std::thread::sleep(std::time::Duration::from_millis(50));
        let after = super::position().unwrap();
        super::nudge(-3, 0);
        eprintln!("指针 ({x}, {y}) → {after:?}，左键按着：{}", super::left_button_down());
        assert!(after.0 > x && after.1 == y, "向右挪了：{after:?}");
        let ws = crate::snap::top_level_windows(std::process::id());
        let w = ws.first().expect("没有窗口");
        assert!(crate::snap::window_owner_pid(w.handle).is_some());
        assert!(super::post_wheel(w.handle, w.rect.x + 5, w.rect.y + 5, 0, false));
    }
}
