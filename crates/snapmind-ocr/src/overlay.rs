//! 原位覆盖（需求第 4 节、M6.2）用的段落块：位置、文字、背景色和文字颜色、字号，以及要不要保持原样。
//!
//! 覆盖块盖住段落里所有行的笔画范围（四周多留 0.25 个字号），背景色和文字颜色取各行测出来的中位数，
//! 字号取各行笔画高度的中位数（界面按它起步，放不下再缩小）。
//!
//! 需求要求公式和表格保持原样。没有版面模型时只能按文字判断：代码块、只有数字和符号的块（价格、
//! 编号、时间）、运算符比例高的块（公式）都不覆盖；表格的单元格各自成段，翻译后原位替换问题不大。

use crate::layout::{is_cjk, is_code_like, paragraph_text, paragraphs};
use crate::Line;

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub text: String,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    /// 原文字号估计（像素）
    pub size: u32,
    /// 保持原样（不翻译、不覆盖）
    pub keep: bool,
}

fn median(mut v: Vec<u32>) -> u32 {
    if v.is_empty() {
        return 0;
    }
    let mid = v.len() / 2;
    *v.select_nth_unstable(mid).1
}

fn median_color(cs: &[[u8; 3]]) -> [u8; 3] {
    let ch = |i: usize| median(cs.iter().map(|c| c[i] as u32).collect()) as u8;
    [ch(0), ch(1), ch(2)]
}

/// 这段文字要不要保持原样：代码、没有文字（只有数字符号）、像公式。
pub fn keep_original(text: &str, lines: &[&Line]) -> bool {
    if lines.len() >= 2 && lines.iter().all(|l| is_code_like(&l.text)) {
        return true;
    }
    let letters = text.chars().filter(|c| c.is_alphabetic() || is_cjk(*c)).count();
    if letters == 0 {
        return true;
    }
    let math = text.chars().filter(|c| "=+−×÷^√∑∫∏≈≠≤≥∞∂±·/<>|".contains(*c)).count();
    let n = text.chars().filter(|c| !c.is_whitespace()).count().max(1);
    // 公式：运算符多、字母少（「x² + y² = 1」「∑ a_i」）
    math * 4 >= n && letters * 2 <= n
}

/// 把识别结果分成覆盖块（按段落，阅读顺序）。坐标和输入的行一致。
pub fn blocks(lines: &[Line], width: u32, height: u32) -> Vec<Block> {
    paragraphs(lines)
        .iter()
        .map(|p| {
            let ls: Vec<&Line> = p.iter().map(|&i| &lines[i]).collect();
            let size = median(ls.iter().map(|l| l.ink.height.max(1)).collect());
            let pad = (size as f32 * 0.25).ceil() as u32;
            let x0 = ls.iter().map(|l| l.ink.left).min().unwrap_or(0).saturating_sub(pad);
            let y0 = ls.iter().map(|l| l.ink.top).min().unwrap_or(0).saturating_sub(pad);
            let x1 = (ls.iter().map(|l| l.ink.right()).max().unwrap_or(0) + pad).min(width);
            let y1 = (ls.iter().map(|l| l.ink.bottom()).max().unwrap_or(0) + pad).min(height);
            let text = paragraph_text(lines, p);
            Block {
                x: x0,
                y: y0,
                width: x1.saturating_sub(x0).max(1),
                height: y1.saturating_sub(y0).max(1),
                keep: keep_original(&text, &ls),
                fg: median_color(&ls.iter().map(|l| l.ink.color).collect::<Vec<_>>()),
                bg: median_color(&ls.iter().map(|l| l.ink.background).collect::<Vec<_>>()),
                size,
                text,
            }
        })
        .collect()
}

/// 把整段译文按空行拆回各块；数目对不上时返回 None（由调用方逐段重译）。
pub fn split_translation(text: &str, n: usize) -> Option<Vec<String>> {
    let parts: Vec<String> = text
        .replace("\r\n", "\n")
        .split("\n\n")
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    (parts.len() == n).then_some(parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ink;

    fn line(text: &str, x: u32, y: u32, w: u32, bg: [u8; 3]) -> Line {
        let ink = Ink { left: x, top: y, width: w, height: 16, color: [20, 20, 20], background: bg };
        Line { text: text.into(), x, y, width: w, height: 16, score: 1.0, ink }
    }

    #[test]
    fn blocks_cover_paragraphs_with_sampled_colors() {
        let lines = vec![
            line("A first paragraph that wraps", 10, 10, 300, [250, 250, 250]),
            line("onto a second page.", 10, 32, 180, [250, 250, 250]),
            line("$19.99", 10, 80, 60, [255, 255, 255]),
        ];
        let b = blocks(&lines, 400, 120);
        assert_eq!(b.len(), 2);
        assert_eq!((b[0].x, b[0].y, b[0].width, b[0].height), (6, 6, 308, 46));
        assert_eq!((b[0].bg, b[0].fg, b[0].size, b[0].keep), ([250, 250, 250], [20, 20, 20], 16, false));
        assert_eq!(b[0].text, "A first paragraph that wraps onto a second page.");
        assert!(b[1].keep, "只有数字符号的块不覆盖");
    }

    #[test]
    fn keeps_formulas_and_code() {
        let l = line("x", 0, 0, 10, [255; 3]);
        assert!(keep_original("x² + y² = 1", &[&l]));
        assert!(keep_original("12:30 / 45%", &[&l]));
        assert!(!keep_original("Save as PDF…", &[&l]));
        assert!(!keep_original("翻译设置 = 默认", &[&l]));
        let (a, b) = (line("fn main() {", 0, 0, 10, [255; 3]), line("}", 0, 20, 10, [255; 3]));
        assert!(keep_original("fn main() {\n}", &[&a, &b]));
    }

    #[test]
    fn splits_translation_back() {
        assert_eq!(split_translation("一\n\n二\r\n\r\n三", 3).unwrap(), vec!["一", "二", "三"]);
        assert_eq!(split_translation("一\n\n\n\n二\n", 2).unwrap(), vec!["一", "二"]);
        assert!(split_translation("一\n二", 2).is_none());
    }
}
