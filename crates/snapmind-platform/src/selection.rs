//! 取当前选中的文字（划词翻译，需求第 5 节、M7.1）。
//!
//! 1. **UI Automation**：问焦点控件要 TextPattern 的选区。原生控件、记事本、Office、WinUI 都支持，
//!    不碰剪贴板。焦点控件支持 TextPattern 但选区是空的，就认为确实没选中（不再模拟复制——VS Code
//!    之类的程序没有选区时 Ctrl+C 会复制整行）。
//! 2. **模拟复制**：拿不到 TextPattern 时（Chromium / Electron 默认不开辅助功能），先存下剪贴板的全部
//!    格式，等用户松开 Alt / Shift / Win，发 Ctrl+C，等剪贴板序号变化后读文字，再把原来的内容放回去。
//!
//! UI Automation 是跨进程调用，个别程序会卡住，所以放在专用线程里、限时等待。
//!
//! macOS 对应：AX 的 `AXSelectedText` 代替 TextPattern，模拟 ⌘C，剪贴板序号用 NSPasteboard 的 changeCount。

use std::time::{Duration, Instant};

/// 取词用的是哪种方法（写日志、调试用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Automation,
    Copy,
}

/// UI Automation 的结果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiaResult {
    /// 拿到了选中的文字
    Text(String),
    /// 控件支持 TextPattern，但没有选中任何文字
    Empty,
    /// 控件不支持 TextPattern，或者超时、出错
    Unsupported,
}

/// UI Automation 最多等多久
const UIA_TIMEOUT: Duration = Duration::from_millis(400);
/// 等用户松开修饰键最多多久
const RELEASE_TIMEOUT: Duration = Duration::from_millis(600);
/// 发出 Ctrl+C 后等剪贴板变化最多多久
const COPY_TIMEOUT: Duration = Duration::from_millis(450);

/// 取选中的文字。`allow_copy` 为 false 时只用 UI Automation。
/// 环境变量 SNAPMIND_SELECTION=copy 时跳过 UI Automation（调试模拟复制用）。
pub fn read(allow_copy: bool) -> Option<(String, Method)> {
    let skip_uia = std::env::var("SNAPMIND_SELECTION").is_ok_and(|v| v == "copy");
    if !skip_uia {
        match imp::uia_selection(UIA_TIMEOUT) {
            UiaResult::Text(t) => return Some((t, Method::Automation)),
            UiaResult::Empty => return None,
            UiaResult::Unsupported => {}
        }
    }
    if !allow_copy {
        return None;
    }
    copy_selection().map(|t| (t, Method::Copy))
}

/// 模拟 Ctrl+C 取词，取完还原剪贴板。
fn copy_selection() -> Option<String> {
    let saved = crate::clipboard::snapshot();
    let before = crate::clipboard::sequence();
    imp::wait_modifiers_released(RELEASE_TIMEOUT);
    imp::send_copy();
    let t0 = Instant::now();
    let mut changed = false;
    while t0.elapsed() < COPY_TIMEOUT {
        std::thread::sleep(Duration::from_millis(10));
        if crate::clipboard::sequence() != before {
            changed = true;
            break;
        }
    }
    if !changed {
        return None; // 没有选中文字，剪贴板也没动过
    }
    // 有的程序分几次写入剪贴板（先清空再放各种格式），稍等一下再读
    std::thread::sleep(Duration::from_millis(30));
    let text = crate::clipboard::text();
    if let Some(s) = saved {
        if let Err(e) = crate::clipboard::restore(&s) {
            eprintln!("划词后还原剪贴板失败：{e}");
        }
    }
    text.map(|t| normalize(&t)).filter(|t| !t.is_empty())
}

/// 统一换行、去掉首尾空白。
pub fn normalize(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").trim().to_string()
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{CUIAutomation, IUIAutomation, IUIAutomationTextPattern, UIA_TextPatternId};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_C, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    pub fn uia_selection(timeout: Duration) -> UiaResult {
        let (tx, rx) = std::sync::mpsc::channel();
        let spawned = std::thread::Builder::new().name("snapmind-selection".into()).spawn(move || {
            let r = unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let r = query();
                CoUninitialize();
                r
            };
            let _ = tx.send(r);
        });
        if spawned.is_err() {
            return UiaResult::Unsupported;
        }
        // 超时就不等了（线程自己跑完退出）
        rx.recv_timeout(timeout).unwrap_or(UiaResult::Unsupported)
    }

    unsafe fn query() -> UiaResult {
        let Ok(automation) = CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER) else {
            return UiaResult::Unsupported;
        };
        let Ok(el) = automation.GetFocusedElement() else { return UiaResult::Unsupported };
        let Ok(pattern) = el.GetCurrentPatternAs::<IUIAutomationTextPattern>(UIA_TextPatternId) else {
            return UiaResult::Unsupported;
        };
        let Ok(ranges) = pattern.GetSelection() else { return UiaResult::Unsupported };
        let n = ranges.Length().unwrap_or(0);
        let mut parts = Vec::new();
        for i in 0..n {
            if let Ok(r) = ranges.GetElement(i) {
                if let Ok(s) = r.GetText(-1) {
                    parts.push(s.to_string());
                }
            }
        }
        let text = normalize(&parts.join("\n"));
        if text.is_empty() {
            UiaResult::Empty
        } else {
            UiaResult::Text(text)
        }
    }

    fn down(vk: VIRTUAL_KEY) -> bool {
        unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: if up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// 快捷键里的 Alt / Shift / Win 还按着时发 Ctrl+C，目标程序收到的是 Ctrl+Alt+C。先等用户松开；
    /// 一直不松就替用户抬起（先发一个没有分配的键，免得单独抬起 Alt 激活菜单栏）。
    pub fn wait_modifiers_released(timeout: Duration) {
        let mods = [VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN];
        let t0 = Instant::now();
        while mods.iter().any(|&m| down(m)) && t0.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(10));
        }
        let held: Vec<VIRTUAL_KEY> = mods.iter().copied().filter(|&m| down(m)).collect();
        if held.is_empty() {
            return;
        }
        let mut inputs = vec![key(VIRTUAL_KEY(0xE8), false), key(VIRTUAL_KEY(0xE8), true)];
        inputs.extend(held.iter().map(|&m| key(m, true)));
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    }

    pub fn send_copy() {
        let inputs = [key(VK_CONTROL, false), key(VK_C, false), key(VK_C, true), key(VK_CONTROL, true)];
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    }
}

/// macOS：先问有焦点的控件要 `AXSelectedText`（需要辅助功能授权）；控件不支持时模拟 ⌘C。
#[cfg(target_os = "macos")]
mod imp {
    use super::*;
    use crate::accessibility::{ax, has_permission};
    use objc2_core_graphics::{CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation};

    /// 美式键盘上 C 键的键码（kVK_ANSI_C）
    const KEY_C: u16 = 8;

    pub fn uia_selection(timeout: Duration) -> UiaResult {
        if !has_permission() {
            return UiaResult::Unsupported;
        }
        // 跨进程调用的超时由 AX 自己控制（AXUIElementSetMessagingTimeout），不用另开线程
        let Some(app) = ax::focused_app(timeout.as_secs_f32()) else { return UiaResult::Unsupported };
        let Some(el) = ax::focused_element(&app) else { return UiaResult::Unsupported };
        match ax::string_attr(&el, "AXSelectedText").map(|t| normalize(&t)) {
            Some(t) if t.is_empty() => UiaResult::Empty,
            Some(t) => UiaResult::Text(t),
            None => UiaResult::Unsupported,
        }
    }

    fn mods_down() -> bool {
        let held = CGEventSource::flags_state(CGEventSourceStateID::CombinedSessionState);
        held.intersects(
            CGEventFlags::MaskShift | CGEventFlags::MaskControl | CGEventFlags::MaskAlternate | CGEventFlags::MaskCommand,
        )
    }

    /// 快捷键（默认 ⌃⌥D）的修饰键还按着时，先等用户松开。等不到也没关系：发出去的 ⌘C 事件
    /// 自带「只有 ⌘」的修饰键状态，目标程序看到的就是 ⌘C。
    pub fn wait_modifiers_released(timeout: Duration) {
        let t0 = Instant::now();
        while mods_down() && t0.elapsed() < timeout {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// 发 ⌘C（要辅助功能授权，没授权时系统静默丢弃，剪贴板不变，取词返回空）。
    pub fn send_copy() {
        for down in [true, false] {
            if let Some(e) = CGEvent::new_keyboard_event(None, KEY_C, down) {
                CGEvent::set_flags(Some(&e), CGEventFlags::MaskCommand);
                CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&e));
            }
        }
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod imp {
    use super::*;

    // Linux 在 M12 实现（主选区 / portal）
    pub fn uia_selection(_timeout: Duration) -> UiaResult {
        UiaResult::Unsupported
    }
    pub fn wait_modifiers_released(_timeout: Duration) {}
    pub fn send_copy() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_line_endings() {
        assert_eq!(normalize("  a\r\nb\rc  \n"), "a\nb\nc");
    }
}

#[cfg(all(test, target_os = "macos"))]
mod mac_tests {
    /// 真机：剪贴板存下 → 写入别的文字 → 还原，前后每一项每种类型都要一样；序号要跟着变。
    #[test]
    #[ignore]
    fn clipboard_round_trip() {
        let before = crate::clipboard::snapshot().unwrap();
        let seq = crate::clipboard::sequence();
        crate::clipboard::set_text("SnapMind 剪贴板测试").unwrap();
        assert_ne!(crate::clipboard::sequence(), seq, "写入后序号要变");
        assert_eq!(crate::clipboard::text().as_deref(), Some("SnapMind 剪贴板测试"));
        crate::clipboard::restore(&before).unwrap();
        let after = crate::clipboard::snapshot().unwrap();
        let kinds: Vec<Vec<&str>> = before.items.iter().map(|i| i.iter().map(|(t, _)| t.as_str()).collect()).collect();
        eprintln!("剪贴板 {} 项，类型 {:?}", before.items.len(), kinds);
        assert_eq!(after.items, before.items);
    }

    /// 真机：问当前焦点控件的选中文字（结果取决于当时的屏幕，只要不卡住、不崩）。
    #[test]
    #[ignore]
    fn reads_focused_selection() {
        let t = std::time::Instant::now();
        let r = super::imp::uia_selection(super::UIA_TIMEOUT);
        eprintln!("AX 选中文字：{r:?}，{:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        assert!(t.elapsed() < std::time::Duration::from_secs(2));
    }
}
