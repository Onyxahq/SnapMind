//! 启动方式判断。

/// 开机自启时附带的参数。带这个参数启动时只进托盘，不弹窗口。
pub const AUTOSTART_FLAG: &str = "--autostart";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// 用户手动打开（开始菜单、双击）：显示设置窗口。
    Interactive,
    /// 开机自启：静默常驻托盘。
    Autostart,
}

/// 根据命令行参数判断启动方式。第一个参数是程序路径，跳过。
pub fn mode<I, S>(args: I) -> LaunchMode
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    if args.into_iter().skip(1).any(|a| a.as_ref() == AUTOSTART_FLAG) {
        LaunchMode::Autostart
    } else {
        LaunchMode::Interactive
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manual_launch_is_interactive() {
        assert_eq!(mode(["snapmind.exe"]), LaunchMode::Interactive);
        assert_eq!(mode(Vec::<String>::new()), LaunchMode::Interactive);
    }

    #[test]
    fn autostart_flag_is_detected() {
        assert_eq!(mode(["snapmind.exe", "--autostart"]), LaunchMode::Autostart);
    }

    #[test]
    fn program_path_is_ignored() {
        // 程序路径本身恰好等于标志时不算。
        assert_eq!(mode(["--autostart"]), LaunchMode::Interactive);
    }
}
