//! 平台抽象层：截屏、全局快捷键、取词、窗口枚举、录屏、剪贴板。
//!
//! 坐标约定：一律使用**物理像素**、虚拟桌面坐标（主显示器左上角为原点，可以为负）。
//! Windows 实现在这里；Linux 实现在 M12。

pub mod bmp;
pub mod capture;
pub mod snap;
pub mod window;

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

/// 虚拟桌面上的矩形，物理像素。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_platform_name() {
        assert!(!platform_name().is_empty());
    }
}
