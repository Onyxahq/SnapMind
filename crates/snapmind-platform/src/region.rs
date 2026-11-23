//! 用户所在地区：只用来给下载挑一个更近的源（下载失败还会换另一个源，判错了也只是慢一点）。

/// 系统地区设成了中国大陆。
///
/// Windows 读「设置 → 时间和语言 → 区域 → 国家或地区」（GEOID 45 = 中国，港澳台各有自己的编号）；
/// 其他平台看 `LC_ALL` / `LANG` 是不是 `zh_CN`。环境变量 `SNAPMIND_REGION=cn` / `other` 可以强制指定。
pub fn mainland_china() -> bool {
    if let Some(v) = std::env::var_os("SNAPMIND_REGION") {
        return v.eq_ignore_ascii_case("cn");
    }
    #[cfg(windows)]
    {
        use windows::Win32::Globalization::{GetUserGeoID, GEOCLASS_NATION};
        // SAFETY: 只读系统设置，没有指针参数
        unsafe { GetUserGeoID(GEOCLASS_NATION) == 45 }
    }
    #[cfg(not(windows))]
    {
        ["LC_ALL", "LANG"].iter().filter_map(std::env::var_os).any(|v| is_zh_cn(&v.to_string_lossy()))
    }
}

#[cfg_attr(windows, allow(dead_code))]
fn is_zh_cn(locale: &str) -> bool {
    let l = locale.to_ascii_lowercase();
    l.starts_with("zh_cn") || l.starts_with("zh-cn") || l.starts_with("zh_hans_cn")
}

#[cfg(test)]
mod tests {
    #[test]
    fn locale_names() {
        assert!(super::is_zh_cn("zh_CN.UTF-8"));
        assert!(!super::is_zh_cn("zh_TW.UTF-8"));
        assert!(!super::is_zh_cn("en_US.UTF-8"));
        // 只要不崩就行：结果取决于本机设置
        let _ = super::mainland_china();
    }
}
