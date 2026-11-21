//! 平台抽象层：截屏、全局快捷键、取词、窗口枚举、录屏、剪贴板。
//!
//! Windows 实现在 M1 开始；Linux 实现在 M12。

/// 当前编译目标平台的名字。
pub fn platform_name() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "other"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_platform_name() {
        assert!(!platform_name().is_empty());
    }
}
