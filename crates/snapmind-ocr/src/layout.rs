//! 行 → 段落（需求 12.1）。M4.3 实现分栏、合并规则和拼接；现在每行单独成段，作为评测的基线。

use crate::Line;

/// 把按阅读顺序排好的行分组成段落，返回每段包含的行下标（按顺序）。
pub fn paragraphs(lines: &[Line]) -> Vec<Vec<usize>> {
    (0..lines.len()).map(|i| vec![i]).collect()
}
