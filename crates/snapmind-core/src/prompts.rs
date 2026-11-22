//! 提示词（需求 1.3）：四套默认提示词、变量渲染。
//!
//! 用户在设置里改过的提示词存在配置的 `prompts` 里，空字符串表示用默认值——
//! 这样以后改进了默认提示词，没改过的用户也能用上。

use serde::{Deserialize, Serialize};

use crate::lang::lang_name;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromptKind {
    /// 识别（视觉大模型做 OCR）
    Recognize,
    /// 翻译（大模型翻译文字，划词翻译也用这一套）
    Translate,
    /// 一步到位（视觉大模型一次完成识别和翻译，输出 JSON）
    Oneshot,
    /// 识别为 Markdown
    Markdown,
}

impl PromptKind {
    pub const ALL: [PromptKind; 4] = [PromptKind::Recognize, PromptKind::Translate, PromptKind::Oneshot, PromptKind::Markdown];
}

pub fn default_prompt(kind: PromptKind) -> &'static str {
    match kind {
        PromptKind::Recognize => "识别图片中的全部文字，按阅读顺序输出。\n- 同一段落内的换行合并成一行，段落之间空一行\n- 保留列表、代码和表格的结构\n- 只输出识别结果，不要解释",
        PromptKind::Translate => "把下面的文字翻译成 {target_lang}。\n- 保持原有的段落、换行和 Markdown 格式\n- 自动判断专业术语和专有名词：保留原文，或使用该领域的通行译法，并在全文保持一致\n- 以下术语保持原样：{glossary}\n- 只输出译文\n\n{text}",
        PromptKind::Oneshot => "识别图片中的全部文字，并翻译成 {target_lang}。\n- 原文按段落整理，段落之间空一行\n- 译文与原文的段落一一对应\n- 自动判断专业术语和专有名词，并在全文保持一致\n- 以下术语保持原样：{glossary}\n- 只输出 JSON，不要解释：{\"source\": \"原文\", \"translation\": \"译文\"}",
        PromptKind::Markdown => "把图片内容整理成 Obsidian 风格的 Markdown：\n- 标题用 #，列表、引用、代码块保持原有结构\n- 表格用管道表格\n- 公式用 $…$ 和 $$…$$（LaTeX）\n- 上下标用 <sup></sup> 和 <sub></sub>，高亮用 ==文字==\n- 只输出 Markdown",
    }
}

/// 渲染时可用的变量。语言用统一代码（`zh-Hans`、`en`……），渲染成语言名。
#[derive(Debug, Clone, Default)]
pub struct PromptVars<'a> {
    pub target_lang: &'a str,
    /// None 或 "auto" 表示自动检测
    pub source_lang: Option<&'a str>,
    /// 不翻译的术语
    pub glossary: &'a [String],
    pub text: &'a str,
    /// 关掉「自动判断专业术语」：删掉含 [`AUTO_TERMS_MARK`] 的行
    pub no_auto_terms: bool,
    /// 关掉「保留格式」：删掉含 [`KEEP_FORMAT_MARK`] 的行
    pub no_keep_format: bool,
}

/// 默认提示词里「自动判断专业术语」那一行的标记文字。
pub const AUTO_TERMS_MARK: &str = "自动判断专业术语";
/// 默认提示词里「保留格式」那一行的标记文字。
pub const KEEP_FORMAT_MARK: &str = "保持原有的段落";

/// 用户的提示词为空（或全是空白）时用默认值。
pub fn effective<'a>(custom: &'a str, kind: PromptKind) -> &'a str {
    if custom.trim().is_empty() {
        default_prompt(kind)
    } else {
        custom
    }
}

/// 渲染提示词：
/// - 只替换 `{target_lang}` `{source_lang}` `{glossary}` `{text}` 四个变量，其他花括号原样保留
///   （一步到位的 JSON 示例里就有花括号）；
/// - 某个变量为空时（没有不翻译的术语、原文语言自动检测），**含这个变量的整行删掉**，
///   不留下「以下术语保持原样：」这种半句；
/// - 翻译提示词里被用户删掉了 `{text}` 时，把原文接在最后，保证模型拿得到原文。
pub fn render(template: &str, kind: PromptKind, vars: &PromptVars) -> String {
    let target = lang_name(vars.target_lang);
    let source = vars.source_lang.map(str::trim).filter(|s| !s.is_empty() && *s != "auto").map(lang_name).unwrap_or_default();
    let glossary = vars.glossary.iter().map(|g| g.trim()).filter(|g| !g.is_empty()).collect::<Vec<_>>().join("、");
    let values: [(&str, &str); 4] =
        [("{target_lang}", &target), ("{source_lang}", &source), ("{glossary}", &glossary), ("{text}", vars.text)];

    let mut lines = Vec::new();
    for line in template.split('\n') {
        // {text} 本身可能是多行，替换后不再参与按行删除
        if values.iter().any(|(k, v)| v.is_empty() && line.contains(k))
            || (vars.no_auto_terms && line.contains(AUTO_TERMS_MARK))
            || (vars.no_keep_format && line.contains(KEEP_FORMAT_MARK))
        {
            continue;
        }
        let mut l = line.to_string();
        for (k, v) in &values {
            l = l.replace(k, v);
        }
        lines.push(l);
    }
    let mut out = lines.join("\n");
    if kind == PromptKind::Translate && !template.contains("{text}") && !vars.text.is_empty() {
        out = format!("{}\n\n{}", out.trim_end(), vars.text);
    }
    out.trim().to_string()
}

/// 解析一步到位的输出：模型可能包上 ```json 代码块或在前后多说几句，
/// 取第一个 `{` 到最后一个 `}` 之间的 JSON，读 `source` 和 `translation`。
pub fn parse_oneshot(output: &str) -> Option<(String, String)> {
    let start = output.find('{')?;
    let end = output.rfind('}')?;
    if end <= start {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&output[start..=end]).ok()?;
    let get = |keys: &[&str]| keys.iter().find_map(|k| v.get(*k).and_then(|x| x.as_str())).map(|s| s.trim().to_string());
    let source = get(&["source", "original", "text"])?;
    let translation = get(&["translation", "translated", "target"])?;
    (!translation.is_empty()).then_some((source, translation))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_defaults_and_drops_lines_with_empty_vars() {
        let g = vec!["Rust".to_string(), " Tauri ".to_string()];
        let v = PromptVars { target_lang: "zh-Hans", source_lang: None, glossary: &g, text: "Hello\nWorld", ..Default::default() };
        let out = render(default_prompt(PromptKind::Translate), PromptKind::Translate, &v);
        assert!(out.starts_with("把下面的文字翻译成 简体中文。"));
        assert!(out.contains("以下术语保持原样：Rust、Tauri"));
        assert!(out.ends_with("只输出译文\n\nHello\nWorld"));

        let none = PromptVars { glossary: &[], ..v.clone() };
        let out = render(default_prompt(PromptKind::Translate), PromptKind::Translate, &none);
        assert!(!out.contains("以下术语"), "没有术语时整行删除：{out}");
        assert!(!out.contains('{'));
    }

    #[test]
    fn json_braces_survive_and_unknown_vars_stay() {
        let v = PromptVars { target_lang: "en", ..Default::default() };
        let out = render(default_prompt(PromptKind::Oneshot), PromptKind::Oneshot, &v);
        assert!(out.contains("翻译成 English"));
        assert!(out.contains("{\"source\": \"原文\", \"translation\": \"译文\"}"));
        assert!(!out.contains("以下术语保持原样"));
        assert_eq!(render("{foo} → {target_lang}", PromptKind::Recognize, &v), "{foo} → English");
    }

    #[test]
    fn source_lang_and_missing_text_var() {
        let v = PromptVars { target_lang: "zh-Hans", source_lang: Some("ja"), glossary: &[], text: "こんにちは", ..Default::default() };
        assert_eq!(render("从 {source_lang} 翻成 {target_lang}", PromptKind::Translate, &v), "从 日语 翻成 简体中文\n\nこんにちは");
        let auto = PromptVars { source_lang: Some("auto"), ..v.clone() };
        assert_eq!(render("原文是 {source_lang}\n翻成 {target_lang}：{text}", PromptKind::Translate, &auto), "翻成 简体中文：こんにちは");
    }

    #[test]
    fn switches_drop_marked_lines() {
        let v = PromptVars { target_lang: "zh-Hans", text: "x", no_auto_terms: true, no_keep_format: true, ..Default::default() };
        let out = render(default_prompt(PromptKind::Translate), PromptKind::Translate, &v);
        assert!(!out.contains(AUTO_TERMS_MARK) && !out.contains(KEEP_FORMAT_MARK), "{out}");
        assert!(out.contains("只输出译文"));
        let g = vec!["SnapMind".to_string()];
        let one = render(default_prompt(PromptKind::Oneshot), PromptKind::Oneshot, &PromptVars { target_lang: "en", glossary: &g, no_auto_terms: true, ..Default::default() });
        assert!(one.contains("以下术语保持原样：SnapMind") && !one.contains(AUTO_TERMS_MARK), "关掉术语判断不影响不翻译列表：{one}");
    }

    #[test]
    fn oneshot_output_parsing() {
        let fenced = "好的：\n```json\n{\"source\": \"Hello\", \"translation\": \"你好\"}\n```";
        assert_eq!(parse_oneshot(fenced), Some(("Hello".into(), "你好".into())));
        assert_eq!(parse_oneshot("{\"original\":\"a\",\"translated\":\" b \"}"), Some(("a".into(), "b".into())));
        assert_eq!(parse_oneshot("{\"source\":\"a\",\"translation\":\"\"}"), None);
        assert_eq!(parse_oneshot("not json"), None);
        assert_eq!(parse_oneshot("} {"), None);
    }

    #[test]
    fn effective_falls_back_to_default() {
        assert_eq!(effective("  \n", PromptKind::Markdown), default_prompt(PromptKind::Markdown));
        assert_eq!(effective("mine", PromptKind::Markdown), "mine");
    }
}
