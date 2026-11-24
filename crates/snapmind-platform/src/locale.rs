//! 系统界面语言：用来在「界面语言：跟随系统」时选择显示语言。

/// 系统首选的界面语言标签（BCP 47，如 `zh-CN`、`en-US`）。读不到时返回空串，调用方当作英语。
pub fn system_ui_language() -> String {
    imp()
}

#[cfg(windows)]
fn imp() -> String {
    use windows::Win32::Globalization::{GetUserPreferredUILanguages, MUI_LANGUAGE_NAME};
    use windows::core::PWSTR;

    let mut count = 0u32;
    let mut len = 0u32;
    // 第一次只问需要多大的缓冲区
    // SAFETY: 传入的指针都指向本函数的局部变量；缓冲区为空时系统只回填长度。
    if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, None, &mut len) }.is_err() || len == 0 {
        return String::new();
    }
    let mut buf = vec![0u16; len as usize];
    // SAFETY: buf 的长度正是上一步系统要求的字符数。
    if unsafe { GetUserPreferredUILanguages(MUI_LANGUAGE_NAME, &mut count, Some(PWSTR(buf.as_mut_ptr())), &mut len) }.is_err() {
        return String::new();
    }
    // 缓冲区是以 \0 分隔、\0\0 结尾的多个标签，取第一个
    let first = buf.split(|c| *c == 0).next().unwrap_or(&[]);
    String::from_utf16_lossy(first)
}

/// macOS：「系统设置 → 通用 → 语言与地区」里排第一的语言（如 `zh-Hans-CN`）。
/// 从访达 / 启动台打开的程序没有 `LANG` 环境变量，所以不能读环境变量。
#[cfg(target_os = "macos")]
fn imp() -> String {
    objc2_foundation::NSLocale::preferredLanguages().firstObject().map(|s| s.to_string()).unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "macos")))]
fn imp() -> String {
    for key in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Ok(v) = std::env::var(key) {
            let tag = v.split('.').next().unwrap_or("").to_string();
            if !tag.is_empty() && tag != "C" && tag != "POSIX" {
                return tag;
            }
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn reads_without_panicking() {
        let tag = super::system_ui_language();
        assert!(!tag.contains('\0'));
    }
}
