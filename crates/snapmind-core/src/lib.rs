//! SnapMind 核心逻辑：配置、路由与备用链、提示词、历史记录。
//!
//! 这个 crate 不依赖 Tauri 和任何平台 API，全部逻辑都可以单元测试。

pub mod config;
pub mod configfile;
pub mod export;
pub mod gifenc;
pub mod history;
pub mod lang;
pub mod prompts;
pub mod stitch;

/// 应用名，用于配置目录、窗口标题等。
pub const APP_NAME: &str = "SnapMind";

/// 应用版本号（来自 workspace 的 Cargo 版本）。
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_semver() {
        let parts: Vec<_> = version().split('.').collect();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.parse::<u32>().is_ok()));
    }
}
