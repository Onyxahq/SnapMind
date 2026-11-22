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

/// 语言「家族」：简繁中文算同一种，其余取主代码（`en-GB` → `en`）。
pub fn family(code: &str) -> String {
    let c = code.trim().to_ascii_lowercase();
    c.split('-').next().unwrap_or("").to_string()
}

/// 粗略判断一段文字的语言，只在有把握时给出结果（用于「原文已是目标语言」）。
///
/// 按文字系统判断：有假名 → 日语，有谚文 → 韩语，以汉字为主 → 中文，西里尔字母 → 俄语，
/// 阿拉伯字母 → 阿拉伯语，泰文 → 泰语；拉丁字母只认英语（常见虚词占比够高），
/// 法语、德语等拉丁字母语言分不清，返回 None，宁可照常翻译也不误判。
pub fn detect(text: &str) -> Option<&'static str> {
    let (mut han, mut kana, mut hangul, mut latin, mut cyr, mut arabic, mut thai) = (0usize, 0, 0, 0, 0, 0, 0);
    for ch in text.chars() {
        match ch as u32 {
            0x4E00..=0x9FFF | 0x3400..=0x4DBF => han += 1,
            0x3040..=0x30FF => kana += 1,
            0xAC00..=0xD7AF | 0x1100..=0x11FF => hangul += 1,
            0x0041..=0x005A | 0x0061..=0x007A | 0x00C0..=0x024F => latin += 1,
            0x0400..=0x04FF => cyr += 1,
            0x0600..=0x06FF => arabic += 1,
            0x0E00..=0x0E7F => thai += 1,
            _ => {}
        }
    }
    let total = han + kana + hangul + latin + cyr + arabic + thai;
    if total < 2 {
        return None;
    }
    let share = |n: usize| n * 100 / total;
    if kana > 0 && share(kana + han) >= 50 {
        return Some("ja");
    }
    if share(hangul) >= 40 {
        return Some("ko");
    }
    if share(han) >= 50 {
        return Some("zh");
    }
    if share(cyr) >= 50 {
        return Some("ru");
    }
    if share(arabic) >= 50 {
        return Some("ar");
    }
    if share(thai) >= 50 {
        return Some("th");
    }
    if share(latin) >= 70 {
        const EN: [&str; 24] = [
            "the", "and", "of", "to", "is", "in", "that", "it", "for", "on", "with", "as", "are", "this", "be", "you", "was",
            "not", "or", "by", "have", "from", "an", "can",
        ];
        let words: Vec<String> = text
            .split(|c: char| !c.is_ascii_alphabetic())
            .filter(|w| !w.is_empty())
            .map(str::to_ascii_lowercase)
            .collect();
        if words.is_empty() {
            return None;
        }
        let hits = words.iter().filter(|w| EN.contains(&w.as_str())).count();
        // 短句里一个虚词也行；长文要占到一成以上
        if (words.len() <= 6 && hits >= 1) || hits * 10 >= words.len() {
            return Some("en");
        }
    }
    None
}

/// 原文是不是已经是目标语言（只在检测有把握时返回 true）。
pub fn is_same_language(text: &str, target: &str) -> bool {
    detect(text).is_some_and(|d| d == family(target))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detection() {
        assert_eq!(detect("翻译不只是替换词语。"), Some("zh"));
        assert_eq!(detect("設定を保存しました"), Some("ja"));
        assert_eq!(detect("설정이 저장되었습니다"), Some("ko"));
        assert_eq!(detect("Translation is not just about swapping words."), Some("en"));
        assert_eq!(detect("Hello world"), None, "太短又没有虚词，不下结论");
        assert_eq!(detect("La traduction n'est pas seulement un échange de mots."), None);
        assert_eq!(detect("Привет, мир"), Some("ru"));
        assert_eq!(detect("1234 !!"), None);
        assert!(is_same_language("你好，世界", "zh-Hans"));
        assert!(is_same_language("你好，世界", "zh-Hant"));
        assert!(!is_same_language("Hello there, this is a test", "zh-Hans"));
        assert!(is_same_language("This is a test of the system", "en"));
        assert_eq!(family("en-GB"), "en");
    }

    #[test]
    fn names() {
        assert_eq!(lang_name("zh-hans"), "简体中文");
        assert_eq!(lang_name("ja"), "日语");
        assert_eq!(lang_name(" xx "), "xx");
    }
}
