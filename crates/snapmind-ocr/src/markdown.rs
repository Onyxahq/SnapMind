//! 本地版面结果 → Markdown（M8.1，识别链里的「本地 OCR」走这里；视觉模型直接按提示词输出 Markdown）。
//!
//! 按段落逐段转换：
//! - 代码块（连续的代码行）→ 围栏代码块，保留缩进；
//! - 列表项（• · ▪ - * 开头）→ `- `，编号（1. 1) 1、）→ `1. `；
//! - 标题：单行、不长、字号明显比正文大——正文字号取所有段落按字数加权的中位数，大 1.8 倍以上 `#`，
//!   大 1.3 倍以上 `##`；只有一种字号时不认标题；
//! - 其余是普通段落。
//!
//! 文字里的 Markdown 特殊字符（行首 `#`、`>`，以及 `*` `_` `` ` `` `[`）转义，免得原文被当成格式。

use crate::layout::{is_list_item, paragraph_text, paragraphs};
use crate::Line;

/// 标题字号比例
const H1: f32 = 1.8;
const H2: f32 = 1.3;
/// 标题最多多少字
const HEADING_MAX_CHARS: usize = 60;

fn em(line: &Line) -> f32 {
    line.ink.height.max(1) as f32 / crate::layout::ink_to_em(&line.text)
}

/// 普通文字里的 Markdown 特殊字符转义。
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        if "\\`*_[]".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    // 行首的 # > 会变成标题、引用
    if out.starts_with('#') || out.starts_with('>') {
        out.insert(0, '\\');
    }
    out
}

/// 去掉列表标记，返回（Markdown 列表前缀，正文）。
fn list_parts(text: &str) -> (String, String) {
    let t = text.trim_start();
    let digits: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let rest = &t[digits.len()..];
        let rest = rest.strip_prefix(['.', ')', '、', '）']).unwrap_or(rest);
        return (format!("{digits}. "), rest.trim_start().to_string());
    }
    let mut chars = t.chars();
    chars.next();
    ("- ".to_string(), chars.as_str().trim_start().to_string())
}

/// 把识别出的行转成 Markdown。
pub fn from_lines(lines: &[Line]) -> String {
    let paras = paragraphs(lines);
    if paras.is_empty() {
        return String::new();
    }
    // 正文字号：各段字号按字数加权的中位数
    let mut sizes: Vec<(f32, usize)> = paras
        .iter()
        .map(|p| {
            let s = p.iter().map(|&i| em(&lines[i])).sum::<f32>() / p.len() as f32;
            let n = p.iter().map(|&i| lines[i].text.chars().count()).sum::<usize>();
            (s, n.max(1))
        })
        .collect();
    sizes.sort_by(|a, b| a.0.total_cmp(&b.0));
    let total: usize = sizes.iter().map(|s| s.1).sum();
    let mut acc = 0;
    let body = sizes.iter().find(|s| {
        acc += s.1;
        acc * 2 >= total
    });
    let body = body.map(|s| s.0).unwrap_or(1.0);

    let mut out: Vec<String> = Vec::new();
    for p in &paras {
        let text = paragraph_text(lines, p);
        if text.is_empty() {
            continue;
        }
        let code = p.len() >= 2 && p.iter().all(|&i| crate::layout::is_code_like(&lines[i].text));
        if code {
            out.push(format!("```\n{text}\n```"));
            continue;
        }
        let first = &lines[p[0]].text;
        if is_list_item(first) {
            let (prefix, rest) = list_parts(&text);
            // 只有标记、没有文字（标记和文字被检测成了两行）：丢掉这个空项
            if !rest.trim().is_empty() {
                out.push(format!("{prefix}{}", escape(&rest)));
            }
            continue;
        }
        let size = p.iter().map(|&i| em(&lines[i])).sum::<f32>() / p.len() as f32;
        let short = p.len() == 1 && text.chars().count() <= HEADING_MAX_CHARS;
        let level = if !short {
            0
        } else if size >= body * H1 {
            1
        } else if size >= body * H2 {
            2
        } else {
            0
        };
        if level > 0 {
            out.push(format!("{} {}", "#".repeat(level), escape(&text)));
        } else {
            out.push(escape(&text));
        }
    }
    // 连续的列表项之间不空行
    let mut md = String::new();
    for (i, block) in out.iter().enumerate() {
        if i > 0 {
            let both_list = is_md_list(&out[i - 1]) && is_md_list(block);
            md.push_str(if both_list { "\n" } else { "\n\n" });
        }
        md.push_str(block);
    }
    md
}

fn is_md_list(block: &str) -> bool {
    block.starts_with("- ") || block.split_once(". ").is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ink;

    fn line(text: &str, x: u32, y: u32, w: u32, h: u32) -> Line {
        let ink = Ink { left: x, top: y, width: w, height: h, color: [0; 3], background: [255; 3] };
        Line { text: text.into(), x, y, width: w, height: h, score: 1.0, ink }
    }

    #[test]
    fn headings_lists_code_and_paragraphs() {
        let v = vec![
            line("快捷键设置", 10, 10, 200, 30),
            line("截图之后按下识别，文字会按段落排好，", 10, 70, 400, 15),
            line("段落内部的换行被去掉。", 10, 92, 300, 15),
            line("截图与贴图", 10, 140, 120, 20),
            line("• Shift+F1 截图", 10, 180, 160, 15),
            line("• Shift+F3 贴图", 10, 206, 160, 15),
            line("1. 打开设置", 10, 240, 160, 15),
            line("fn main() {", 10, 290, 110, 15),
            line("}", 10, 312, 10, 15),
        ];
        assert_eq!(
            from_lines(&v),
            "# 快捷键设置\n\n截图之后按下识别，文字会按段落排好，段落内部的换行被去掉。\n\n## 截图与贴图\n\n- Shift+F1 截图\n- Shift+F3 贴图\n1. 打开设置\n\n```\nfn main() {\n}\n```"
        );
    }

    #[test]
    fn single_size_has_no_headings_and_escapes() {
        let v = vec![line("# not a heading", 10, 10, 200, 15), line("Use *stars* and [links]", 10, 60, 200, 15)];
        assert_eq!(from_lines(&v), "\\# not a heading\n\nUse \\*stars\\* and \\[links\\]");
        assert_eq!(from_lines(&[]), "");
        // 只有列表标记的一行不输出空列表项
        assert_eq!(from_lines(&[line("- ", 10, 10, 10, 15), line("正文", 40, 60, 40, 15)]), "正文");
    }
}
