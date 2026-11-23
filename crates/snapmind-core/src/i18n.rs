//! 后端文案的多语言。源语言是简体中文：代码里直接写中文，`tr("中文")` 查当前语言的词典，
//! 查不到就原样返回。词典在 `locales/<语言>.json`（键是中文原文），编译时嵌进程序。
//!
//! 带变量的句子用 `{name}`：`trf("保存失败：{e}", &[("e", &err)])`。
//! 前端有自己的一套（`ui/src/locales`），两边共用同一个工具维护，见 `scripts/i18n.mjs`。

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::config::Language;

type Dict = Arc<HashMap<String, String>>;

static CURRENT: RwLock<Option<(Language, Dict)>> = RwLock::new(None);

fn source(lang: Language) -> Option<&'static str> {
    Some(match lang {
        Language::System | Language::ZhCn => return None,
        Language::ZhTw => include_str!("../locales/zh-TW.json"),
        Language::En => include_str!("../locales/en.json"),
        Language::Ja => include_str!("../locales/ja.json"),
        Language::Ko => include_str!("../locales/ko.json"),
        Language::Es => include_str!("../locales/es.json"),
        Language::Fr => include_str!("../locales/fr.json"),
        Language::De => include_str!("../locales/de.json"),
        Language::Ru => include_str!("../locales/ru.json"),
        Language::PtBr => include_str!("../locales/pt-BR.json"),
    })
}

fn load(lang: Language) -> Dict {
    let map = source(lang).and_then(|s| serde_json::from_str::<HashMap<String, String>>(s).ok()).unwrap_or_default();
    Arc::new(map)
}

/// 设置后端文案的语言。`System` 按系统语言（`system_tag`，如 `zh-CN`）解析，返回实际使用的语言。
pub fn set_language(lang: Language, system_tag: &str) -> Language {
    let lang = if lang == Language::System { Language::from_tag(system_tag) } else { lang };
    let mut cur = CURRENT.write().expect("i18n 锁");
    if cur.as_ref().is_none_or(|(l, _)| *l != lang) {
        *cur = Some((lang, load(lang)));
    }
    lang
}

/// 当前语言；还没设置过时是简体中文。
pub fn language() -> Language {
    CURRENT.read().expect("i18n 锁").as_ref().map_or(Language::ZhCn, |(l, _)| *l)
}

/// 翻译：key 是中文原文，没有译文时原样返回。
pub fn tr(key: &str) -> String {
    let cur = CURRENT.read().expect("i18n 锁");
    cur.as_ref().and_then(|(_, d)| d.get(key)).map_or_else(|| key.to_string(), |s| s.clone())
}

/// 翻译并替换 `{name}`；没有对应参数的占位符原样保留。
pub fn trf(key: &str, args: &[(&str, &str)]) -> String {
    format_args_into(&tr(key), args)
}

/// 把 `{name}` 换成参数值（单独拿出来方便测试）。
pub fn format_args_into(text: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        match after.find('}') {
            Some(close) if after[..close].chars().all(|c| c.is_alphanumeric() || c == '_') && close > 0 => {
                let name = &after[..close];
                match args.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => out.push_str(v),
                    None => {
                        out.push('{');
                        out.push_str(name);
                        out.push('}');
                    }
                }
                rest = &after[close + 1..];
            }
            _ => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_replaced_and_unknown_ones_kept() {
        assert_eq!(format_args_into("保存失败：{e}", &[("e", "磁盘满")]), "保存失败：磁盘满");
        assert_eq!(format_args_into("{a} → {b}", &[("a", "1")]), "1 → {b}");
        assert_eq!(format_args_into("没有占位符", &[]), "没有占位符");
        assert_eq!(format_args_into("括号 { 不成对", &[]), "括号 { 不成对");
        assert_eq!(format_args_into("{}", &[]), "{}");
    }

    #[test]
    fn every_locale_file_is_a_flat_string_map() {
        for lang in [Language::ZhTw, Language::En, Language::Ja, Language::Ko, Language::Es, Language::Fr, Language::De, Language::Ru, Language::PtBr] {
            let text = source(lang).unwrap();
            serde_json::from_str::<HashMap<String, String>>(text).unwrap_or_else(|e| panic!("{lang:?}: {e}"));
        }
    }
}
