//! 语言：统一用 BCP 47 风格的代码（`zh-Hans`、`zh-Hant`、`en`……），
//! 提示词里渲染成语言名，设置页的目标语言下拉框也用这张表。

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Lang {
    pub code: &'static str,
    /// 界面上显示的名字（同时用于提示词）
    pub name: &'static str,
}

/// 常用语言，按使用频率排序。
pub const LANGS: [Lang; 16] = [
    Lang { code: "zh-Hans", name: "简体中文" },
    Lang { code: "zh-Hant", name: "繁體中文" },
    Lang { code: "en", name: "English" },
    Lang { code: "ja", name: "日语" },
    Lang { code: "ko", name: "韩语" },
    Lang { code: "fr", name: "法语" },
    Lang { code: "de", name: "德语" },
    Lang { code: "es", name: "西班牙语" },
    Lang { code: "ru", name: "俄语" },
    Lang { code: "pt", name: "葡萄牙语" },
    Lang { code: "it", name: "意大利语" },
    Lang { code: "ar", name: "阿拉伯语" },
    Lang { code: "vi", name: "越南语" },
    Lang { code: "th", name: "泰语" },
    Lang { code: "id", name: "印尼语" },
    Lang { code: "tr", name: "土耳其语" },
];

/// 代码 → 语言名；不认识的代码原样返回（大模型通常也看得懂）。
pub fn lang_name(code: &str) -> String {
    let c = code.trim();
    LANGS
        .iter()
        .find(|l| l.code.eq_ignore_ascii_case(c))
        .map(|l| l.name.to_string())
        .unwrap_or_else(|| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(lang_name("zh-hans"), "简体中文");
        assert_eq!(lang_name("ja"), "日语");
        assert_eq!(lang_name(" xx "), "xx");
    }
}
