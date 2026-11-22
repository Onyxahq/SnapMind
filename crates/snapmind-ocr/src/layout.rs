//! 行 → 段落（需求 12.1）。
//!
//! 不单独做分栏：每一行只和「正上方最近、所在段落和它横向有重叠」的那一行比较，别的栏、右侧按钮天然不会被拿来比。
//! 两行能合进同一段的条件（全部满足）：
//! - **样式一致**：字号接近（用笔画的实际高度，不用外扩过的检测框）、文字颜色和背景颜色接近；
//! - **不是新的列表项**：下一行不以 •、1.、(1) 之类的标记开头；
//! - **左边缘对齐**：允许首行缩进的段落（第一行比第二行靠右），下一行比上一行缩进则视为新段落；居中排版按中线对齐；
//! - **行距**：笔画之间的空隙不超过字高的 [`GAP_MAX`] 倍，而且没有比这段前面的行距突然变大。
//!
//! 代码（行尾是 `{ } ;`、以 `//` 开头之类）连续出现时按代码块处理：不看缩进，拼接时保留换行和缩进。
//! 段落最后按版面排序（先上下分行、再左右分栏，递归），双栏文章是先左栏后右栏。

use crate::ink::color_distance;
use crate::Line;

/// 笔画间空隙 / 字号的上限（约等于行距 2.15 倍）。样本集里最松的正文（2 倍行距）是 1.09，
/// 最紧的菜单（11 px 字、每项 26 px）是 1.29，取中间。
pub const GAP_MAX: f32 = 1.2;
/// 与这段前面的行距相比，空隙增加超过「× 倍数 + 常数」算行距突变。
const GAP_JUMP_RATIO: f32 = 1.35;
const GAP_JUMP_ADD: f32 = 0.3;
/// 字号之比超过它算字号变化。
const SIZE_RATIO: f32 = 1.25;
/// 文字颜色、背景颜色的差（各通道最大差）超过它算样式变化。
const COLOR_TOL: u8 = 48;
const BACKGROUND_TOL: u8 = 24;
/// 左边缘对齐的容差（× 字号）。
const ALIGN_TOL: f32 = 0.6;
/// 首行缩进最多几个字。
const INDENT_MAX: f32 = 4.0;
/// 代码块里两行之间的空隙上限（× 字号）。
const CODE_GAP_MAX: f32 = 1.2;

/// 一行用于版面判断的几何和文字特征。
#[derive(Debug, Clone, Copy)]
struct Geo {
    l: f32,
    t: f32,
    r: f32,
    b: f32,
    /// 字号估计（像素）：笔画高度按这一行有哪些字母换算，见 [`ink_to_em`]
    g: f32,
    color: [u8; 3],
    bg: [u8; 3],
    list: bool,
    code: bool,
}

impl Geo {
    fn of(line: &Line) -> Self {
        let k = &line.ink;
        let (l, t, w, h) = if k.height > 0 && k.width > 0 {
            (k.left, k.top, k.width, k.height)
        } else {
            (line.x, line.y, line.width, line.height)
        };
        Geo {
            l: l as f32,
            t: t as f32,
            r: (l + w) as f32,
            b: (t + h) as f32,
            g: h.max(1) as f32 / ink_to_em(&line.text),
            color: k.color,
            bg: k.background,
            list: is_list_item(&line.text),
            code: is_code_like(&line.text),
        }
    }
    fn cx(&self) -> f32 {
        (self.l + self.r) / 2.0
    }
}

/// 是否以列表标记开头：• · ▪ - * ✓、1. 1) 1、 (1) （1） ① 一、 a)
pub fn is_list_item(text: &str) -> bool {
    let t = text.trim_start();
    let mut chars = t.chars();
    let Some(c0) = chars.next() else { return false };
    let rest: String = chars.clone().collect();
    if "•·・▪▫●○◦■□◆◇►▶➤✓✔☐☑※".contains(c0) {
        return true;
    }
    // 短横线、星号后面要跟空格（避免负数、强调）
    if "-–—*+".contains(c0) {
        return rest.starts_with(' ');
    }
    if ('①'..='⑳').contains(&c0) {
        return true;
    }
    // (1) （1） (a)
    if c0 == '(' || c0 == '（' {
        let inner: String = rest
            .chars()
            .take_while(|c| *c != ')' && *c != '）')
            .collect();
        return !inner.is_empty()
            && inner.chars().count() <= 3
            && inner
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "一二三四五六七八九十".contains(c))
            && rest.len() > inner.len();
    }
    // 1. 1) 1、 12. a) 一、
    let head: String = t.chars().take_while(|c| c.is_ascii_digit()).collect();
    let after = |n: usize| t.chars().nth(n);
    if !head.is_empty() && head.len() <= 3 {
        return match after(head.len()) {
            Some('.') | Some(')') => {
                after(head.len() + 1).is_some_and(|c| c == ' ' || !c.is_ascii_digit())
            }
            Some('、') | Some('）') => true,
            _ => false,
        };
    }
    if c0.is_ascii_lowercase() && after(1) == Some(')') {
        return true;
    }
    if "一二三四五六七八九十".contains(c0) && after(1) == Some('、') {
        return true;
    }
    false
}

/// 像不像一行代码：行尾 `{ } ;`、以 `//` `#!` 开头、或者括号和运算符比例高。
pub fn is_code_like(text: &str) -> bool {
    let t = text.trim();
    if t.is_empty() {
        return false;
    }
    if t.ends_with(['{', '}', ';'])
        || t.starts_with("//")
        || t.starts_with("/*")
        || t.starts_with("#!")
    {
        return true;
    }
    let n = t.chars().count();
    let sym = t.chars().filter(|c| "{}[]()<>=;".contains(*c)).count();
    n >= 6 && sym * 6 >= n && !t.chars().any(is_cjk)
}

/// 一行笔画高度占字号的比例，按字母形状估计：中日韩文字、括号几乎占满字身；拉丁字母要看有没有
/// 上伸部（b d f h k l t、大写、数字）和下伸部（g j p q y）。没有下伸部的英文行笔画只有字号的约 3/4，
/// 直接拿笔画高度比字号会把同一字号的两行当成不同字号。
pub fn ink_to_em(text: &str) -> f32 {
    let (mut full, mut asc, mut desc, mut any) = (false, false, false, false);
    for c in text.chars() {
        if is_cjk(c) || "()[]{}|/\\@$".contains(c) {
            full = true;
        } else if c.is_ascii_uppercase() || c.is_ascii_digit() || "bdfhiklt!?#%&".contains(c) {
            asc = true;
        } else if "gjpqy".contains(c) {
            desc = true;
        } else if c.is_alphanumeric() {
            any = true;
        }
    }
    match (full || (asc && desc), asc, desc) {
        (true, _, _) => 0.95,
        (_, true, _) => 0.75,
        (_, _, true) => 0.72,
        _ if any => 0.5,
        _ => 0.95,
    }
}

/// 中日韩文字（含全角标点）：拼接时不加空格。
pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FFF | 0x3000..=0x303F | 0x3040..=0x30FF | 0x3100..=0x31FF | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFE30..=0xFE4F | 0xFF00..=0xFFEF)
}

fn can_merge(geo: &[Geo], para: &[usize], u: usize, l: usize) -> bool {
    let (a, b) = (&geo[u], &geo[l]);
    let gm = a.g.min(b.g);
    let em = a.g.max(b.g);
    if em / gm > SIZE_RATIO
        || color_distance(a.color, b.color) > COLOR_TOL
        || color_distance(a.bg, b.bg) > BACKGROUND_TOL
        || b.list
    {
        return false;
    }
    let gap = (b.t - a.b) / em;
    let tol = ALIGN_TOL * em;
    if a.code && b.code {
        let left = para.iter().map(|&i| geo[i].l).fold(f32::MAX, f32::min);
        return gap <= CODE_GAP_MAX && b.l >= left - tol;
    }
    let first = para.len() == 1;
    let dl = b.l - a.l;
    let aligned = dl.abs() <= tol
        // 首行缩进：第一行靠右，第二行顶格
        || (first && dl < 0.0 && -dl <= INDENT_MAX * em)
        // 列表项的悬挂缩进：标记后面的文字换行后和文字对齐
        || (first && a.list && dl > 0.0 && dl <= 3.0 * em)
        // 居中排版
        || (a.cx() - b.cx()).abs() <= tol;
    if !aligned || gap > GAP_MAX {
        return false;
    }
    // 行距突变：和这段里上一对行的空隙比
    if para.len() >= 2 {
        let p = &geo[para[para.len() - 2]];
        let prev_gap = (a.t - p.b) / p.g.max(a.g);
        if gap > prev_gap * GAP_JUMP_RATIO + GAP_JUMP_ADD {
            return false;
        }
    }
    true
}

/// 把行分组成段落，返回每段包含的行下标（段内按从上到下，段落按版面阅读顺序）。
/// 传入任意一部分行（例如右键划选到的行）也可以，按同样的规则重新分段。
pub fn paragraphs(lines: &[Line]) -> Vec<Vec<usize>> {
    let geo: Vec<Geo> = lines.iter().map(Geo::of).collect();
    let mut order: Vec<usize> = (0..lines.len()).collect();
    order.sort_by(|&i, &j| {
        geo[i]
            .t
            .total_cmp(&geo[j].t)
            .then(geo[i].l.total_cmp(&geo[j].l))
    });

    let mut paras: Vec<Vec<usize>> = Vec::new();
    let mut para_of = vec![usize::MAX; lines.len()];
    let mut has_next = vec![false; lines.len()];
    // 每段的左右范围
    let mut extent: Vec<(f32, f32)> = Vec::new();
    for (k, &i) in order.iter().enumerate() {
        let b = &geo[i];
        // 正上方最近的一行：它所在的段落和这一行横向有重叠（代码的右括号比上一行靠左也算），
        // 下边缘不低于这一行的上边缘（允许挤进字高的 40%）
        let above = order[..k]
            .iter()
            .copied()
            .filter(|&j| {
                let (a, e) = (&geo[j], &extent[para_of[j]]);
                a.b <= b.t + 0.4 * a.g.min(b.g) && e.1.min(b.r) > e.0.max(b.l)
            })
            .max_by(|&x, &y| geo[x].b.total_cmp(&geo[y].b));
        match above {
            Some(u) if !has_next[u] && can_merge(&geo, &paras[para_of[u]], u, i) => {
                has_next[u] = true;
                let p = para_of[u];
                para_of[i] = p;
                paras[p].push(i);
                extent[p] = (extent[p].0.min(b.l), extent[p].1.max(b.r));
            }
            _ => {
                para_of[i] = paras.len();
                paras.push(vec![i]);
                extent.push((b.l, b.r));
            }
        }
    }

    // 段落按版面排序
    let boxes: Vec<[f32; 4]> = paras
        .iter()
        .map(|p| {
            p.iter()
                .fold([f32::MAX, f32::MAX, f32::MIN, f32::MIN], |m, &i| {
                    let g = &geo[i];
                    [m[0].min(g.l), m[1].min(g.t), m[2].max(g.r), m[3].max(g.b)]
                })
        })
        .collect();
    let mut idx: Vec<usize> = (0..paras.len()).collect();
    let mut out = Vec::with_capacity(paras.len());
    xy_cut(&boxes, &mut idx, true, false, &mut out);
    out.into_iter()
        .map(|i| std::mem::take(&mut paras[i]))
        .collect()
}

/// 递归 XY 切分：先找横向的空白带把块分成上下几行，再在每行里找纵向空白带分成左右几栏。
/// 一个方向切不开就换另一个方向；两个方向都切不开时按从上到下、从左到右。
fn xy_cut(
    boxes: &[[f32; 4]],
    idx: &mut [usize],
    horizontal: bool,
    other_tried: bool,
    out: &mut Vec<usize>,
) {
    if idx.len() <= 1 {
        out.extend_from_slice(idx);
        return;
    }
    let (lo, hi) = if horizontal { (1, 3) } else { (0, 2) };
    idx.sort_by(|&a, &b| boxes[a][lo].total_cmp(&boxes[b][lo]));
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut end = f32::MIN;
    for &i in idx.iter() {
        if groups.is_empty() || boxes[i][lo] >= end {
            groups.push(Vec::new());
        }
        end = end.max(boxes[i][hi]);
        groups.last_mut().unwrap().push(i);
    }
    if groups.len() == 1 {
        if other_tried {
            idx.sort_by(|&a, &b| {
                boxes[a][1]
                    .total_cmp(&boxes[b][1])
                    .then(boxes[a][0].total_cmp(&boxes[b][0]))
            });
            out.extend_from_slice(idx);
        } else {
            xy_cut(boxes, idx, !horizontal, true, out);
        }
        return;
    }
    for mut g in groups {
        xy_cut(boxes, &mut g, !horizontal, false, out);
    }
}

/// 段落的文字：中日韩直接拼接，拉丁文补空格，行尾连字符拆掉；代码块保留换行和缩进。
pub fn paragraph_text(lines: &[Line], para: &[usize]) -> String {
    let geo: Vec<Geo> = para.iter().map(|&i| Geo::of(&lines[i])).collect();
    if para.len() >= 2 && geo.iter().all(|g| g.code) {
        // 代码：按字宽把左边缘换算成缩进空格
        let left = geo.iter().map(|g| g.l).fold(f32::MAX, f32::min);
        let mut widths: Vec<f32> = para
            .iter()
            .zip(&geo)
            .filter_map(|(&i, g)| {
                let n = lines[i].text.trim().chars().count();
                (n >= 4).then(|| (g.r - g.l) / n as f32)
            })
            .collect();
        widths.sort_by(f32::total_cmp);
        let cw = widths
            .get(widths.len() / 2)
            .copied()
            .unwrap_or(geo[0].g * 0.55)
            .max(1.0);
        return para
            .iter()
            .zip(&geo)
            .map(|(&i, g)| {
                format!(
                    "{}{}",
                    " ".repeat(((g.l - left) / cw).round() as usize),
                    lines[i].text.trim()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    let mut out = String::new();
    for &i in para {
        let t = lines[i].text.trim();
        if t.is_empty() {
            continue;
        }
        let first = t.chars().next().unwrap();
        if let Some(last) = out.chars().last() {
            let before_last = out.chars().rev().nth(1);
            if last == '-'
                && before_last.is_some_and(|c| c.is_alphabetic() && !is_cjk(c))
                && first.is_lowercase()
            {
                out.pop(); // 行尾连字符：hyph- + enation → hyphenation
            } else if !(is_cjk(last) || is_cjk(first)) {
                out.push(' ');
            }
        }
        out.push_str(t);
    }
    out
}

/// 把选中的一部分行（下标，顺序随意）按段落规则重新分段并拼成文字（需求 12.1 第 6 条）。
/// 按段落：段落之间空一行；按行：一行一行，顺序和按段落时一致。
pub fn compose(lines: &[Line], pick: &[usize], by_line: bool) -> String {
    let mut pick: Vec<usize> = pick.iter().copied().filter(|&i| i < lines.len()).collect();
    pick.sort_unstable();
    pick.dedup();
    let subset: Vec<Line> = pick.iter().map(|&i| lines[i].clone()).collect();
    let paras = paragraphs(&subset);
    if by_line {
        return paras.iter().flatten().map(|&i| subset[i].text.trim()).collect::<Vec<_>>().join("\n");
    }
    paras.iter().map(|p| paragraph_text(&subset, p)).filter(|t| !t.is_empty()).collect::<Vec<_>>().join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ink;

    /// 一行：笔画范围直接给定，黑字白底
    fn line(text: &str, x: u32, y: u32, w: u32, h: u32) -> Line {
        Line {
            text: text.into(),
            x,
            y,
            width: w,
            height: h,
            score: 1.0,
            ink: Ink {
                left: x,
                top: y,
                width: w,
                height: h,
                color: [0; 3],
                background: [255; 3],
            },
        }
    }

    fn texts(lines: &[Line]) -> Vec<String> {
        paragraphs(lines)
            .iter()
            .map(|p| paragraph_text(lines, p))
            .collect()
    }

    #[test]
    fn gaps_alignment_and_joining() {
        let v = vec![
            line("Lines whose gap is small are hyph-", 10, 10, 300, 14),
            line("enated and merged,", 10, 30, 160, 14),
            line("as one paragraph.", 10, 50, 150, 14),
            // 空隙 2 倍字高：新段落
            line("新的段落，中文直接", 10, 92, 140, 14),
            line("拼接。", 10, 112, 40, 14),
        ];
        assert_eq!(
            texts(&v),
            vec![
                "Lines whose gap is small are hyphenated and merged, as one paragraph.",
                "新的段落，中文直接拼接。"
            ]
        );
    }

    #[test]
    fn indent_list_size_color_and_jump() {
        let v = vec![
            // 首行缩进的段落
            line("首行缩进两个字的段落，", 38, 10, 280, 14),
            line("第二行顶格。", 10, 30, 90, 14),
            // 下一行缩进：新段落（行距不变也一样）
            line("  缩进开始新段落", 38, 50, 200, 14),
            // 列表项各自成段
            line("• 第一项", 10, 70, 80, 14),
            line("• 第二项", 10, 90, 80, 14),
        ];
        assert_eq!(
            texts(&v),
            vec![
                "首行缩进两个字的段落，第二行顶格。",
                "缩进开始新段落",
                "• 第一项",
                "• 第二项"
            ]
        );

        // 字号变化、颜色变化、行距突变
        let mut desc = line("说明文字", 10, 52, 100, 14);
        desc.ink.color = [120, 120, 120];
        let v = vec![
            line("大标题", 10, 0, 90, 22),
            line("标签", 10, 34, 60, 14),
            desc,
            line("A", 10, 70, 20, 14),
            line("B", 10, 90, 20, 14),
            line("C", 10, 124, 20, 14),
        ];
        let p = texts(&v);
        assert_eq!(p, vec!["大标题", "标签", "说明文字", "A B", "C"]);
    }

    #[test]
    fn columns_and_code() {
        let v = vec![
            line("Left column first line", 10, 10, 200, 14),
            line("Right column first line", 300, 11, 200, 14),
            line("left column second line", 10, 30, 200, 14),
            line("right column second line", 300, 31, 200, 14),
        ];
        assert_eq!(
            texts(&v),
            vec![
                "Left column first line left column second line",
                "Right column first line right column second line"
            ]
        );

        let v = vec![
            line("fn main() {", 10, 10, 110, 14),
            // 没有下伸部的行笔画矮一些（字号相同）
            line("let x = 1;", 50, 31, 100, 11),
            line("}", 10, 50, 10, 14),
        ];
        assert_eq!(texts(&v), vec!["fn main() {\n    let x = 1;\n}"]);
    }

    #[test]
    fn compose_subsets() {
        let v = vec![
            line("第一段第一行，", 10, 10, 200, 14),
            line("第一段第二行。", 10, 30, 200, 14),
            line("Second paragraph", 10, 72, 200, 14),
            line("goes on here.", 10, 92, 200, 14),
        ];
        assert_eq!(compose(&v, &[0, 1, 2, 3], false), "第一段第一行，第一段第二行。\n\nSecond paragraph goes on here.");
        assert_eq!(compose(&v, &[3, 1], false), "第一段第二行。\n\ngoes on here.");
        assert_eq!(compose(&v, &[0, 1, 2, 3], true), "第一段第一行，\n第一段第二行。\nSecond paragraph\ngoes on here.");
        assert_eq!(compose(&v, &[], false), "");
    }

    #[test]
    fn list_markers() {
        for t in [
            "• a",
            "· a",
            "1. a",
            "12) a",
            "（3）步骤",
            "① a",
            "一、总则",
            "- a",
            "a) b",
        ] {
            assert!(is_list_item(t), "{t}");
        }
        for t in [
            "-5 度",
            "1.5 倍",
            "Apple",
            "2026 年",
            "(see above)",
            "*强调*",
        ] {
            assert!(!is_list_item(t), "{t}");
        }
        assert!(is_code_like("let text = ocr.recognize(image)?;"));
        assert!(is_code_like("// Run: cargo run"));
        assert!(!is_code_like(
            "A good result keeps the paragraphs, (mostly)."
        ));
    }
}
