//! OCR 评测：字符错误率、行召回率、段落 F1（编程计划 M4.2）。
//!
//! 真值是「段落 → 行」两层结构（样本 .txt：一行一行，段落之间空一行）。
//! 比较时去掉所有空白（模型会省略中英文之间的空格，段落拼接时再补），省略号统一成「...」。
//!
//! - **行匹配**：预测行和真值行一对一贪心配对（每次取相对编辑距离最小的一对，超过一半就不算配对），
//!   这样双栏、按钮在标签右边之类的阅读顺序差异不会被算成错字。
//! - **字符错误率**：配对行的编辑距离 + 没配上的真值行全部字数 + 多出来的预测行全部字数，除以真值总字数。
//! - **行召回率**：配上且这一行的错误率 ≤ 10% 的真值行占比。
//! - **段落 F1**：预测段落和真值段落同样贪心配对，拼起来的文字错误率 ≤ 10% 算命中。

use std::path::{Path, PathBuf};

/// 一行算「认对了」的错误率上限，段落同理。
pub const LINE_OK: f64 = 0.10;

/// 比较用的规范化：去空白，省略号统一。
pub fn normalize(s: &str) -> Vec<char> {
    s.replace('…', "...").chars().filter(|c| !c.is_whitespace()).collect()
}

/// 编辑距离（插入、删除、替换各算 1）。
pub fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + usize::from(ca != cb));
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// 一对一贪心配对：返回 (真值下标, 预测下标, 编辑距离)。相对距离超过 0.5 的不配。
pub fn match_greedy(truth: &[Vec<char>], pred: &[Vec<char>]) -> Vec<(usize, usize, usize)> {
    let mut cands = Vec::new();
    for (i, t) in truth.iter().enumerate() {
        for (j, p) in pred.iter().enumerate() {
            let d = levenshtein(t, p);
            let rel = d as f64 / t.len().max(p.len()).max(1) as f64;
            if rel <= 0.5 {
                cands.push((rel, i, j, d));
            }
        }
    }
    cands.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    let (mut ut, mut up) = (vec![false; truth.len()], vec![false; pred.len()]);
    let mut out = Vec::new();
    for (_, i, j, d) in cands {
        if !ut[i] && !up[j] {
            ut[i] = true;
            up[j] = true;
            out.push((i, j, d));
        }
    }
    out
}

/// 一个样本的计数（可以累加成总体）。
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Counts {
    pub edits: usize,
    pub truth_chars: usize,
    pub truth_lines: usize,
    pub pred_lines: usize,
    pub recalled_lines: usize,
    pub truth_paras: usize,
    pub pred_paras: usize,
    pub para_hits: usize,
}

impl Counts {
    pub fn add(&mut self, o: &Counts) {
        self.edits += o.edits;
        self.truth_chars += o.truth_chars;
        self.truth_lines += o.truth_lines;
        self.pred_lines += o.pred_lines;
        self.recalled_lines += o.recalled_lines;
        self.truth_paras += o.truth_paras;
        self.pred_paras += o.pred_paras;
        self.para_hits += o.para_hits;
    }
    pub fn cer(&self) -> f64 {
        self.edits as f64 / self.truth_chars.max(1) as f64
    }
    pub fn line_recall(&self) -> f64 {
        self.recalled_lines as f64 / self.truth_lines.max(1) as f64
    }
    pub fn para_f1(&self) -> f64 {
        let p = self.para_hits as f64 / self.pred_paras.max(1) as f64;
        let r = self.para_hits as f64 / self.truth_paras.max(1) as f64;
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

/// 评测一个样本。`pred_lines` 是按阅读顺序的预测行，`pred_paras` 是段落（每段是行下标，按顺序）。
pub fn evaluate(truth: &[Vec<String>], pred_lines: &[String], pred_paras: &[Vec<usize>]) -> Counts {
    let t_lines: Vec<Vec<char>> = truth.iter().flatten().map(|l| normalize(l)).filter(|l| !l.is_empty()).collect();
    let p_lines: Vec<Vec<char>> = pred_lines.iter().map(|l| normalize(l)).collect();
    let pairs = match_greedy(&t_lines, &p_lines);
    let (mut used_t, mut used_p) = (vec![false; t_lines.len()], vec![false; p_lines.len()]);
    let mut edits = 0;
    let mut recalled = 0;
    for &(i, j, d) in &pairs {
        used_t[i] = true;
        used_p[j] = true;
        edits += d;
        if d as f64 <= LINE_OK * t_lines[i].len() as f64 {
            recalled += 1;
        }
    }
    edits += t_lines.iter().zip(&used_t).filter(|(_, u)| !**u).map(|(l, _)| l.len()).sum::<usize>();
    edits += p_lines.iter().zip(&used_p).filter(|(_, u)| !**u).map(|(l, _)| l.len()).sum::<usize>();

    let t_paras: Vec<Vec<char>> = truth.iter().map(|p| normalize(&p.concat())).filter(|p| !p.is_empty()).collect();
    let p_paras: Vec<Vec<char>> = pred_paras
        .iter()
        .map(|idx| normalize(&idx.iter().map(|&i| pred_lines[i].as_str()).collect::<String>()))
        .filter(|p| !p.is_empty())
        .collect();
    let para_hits = match_greedy(&t_paras, &p_paras)
        .into_iter()
        .filter(|&(i, _, d)| d as f64 <= LINE_OK * t_paras[i].len() as f64)
        .count();

    Counts {
        edits,
        truth_chars: t_lines.iter().map(Vec::len).sum(),
        truth_lines: t_lines.len(),
        pred_lines: p_lines.len(),
        recalled_lines: recalled,
        truth_paras: t_paras.len(),
        pred_paras: p_paras.len(),
        para_hits,
    }
}

/// 一个评测样本：图片 + 真值。
pub struct Sample {
    pub name: String,
    pub image: PathBuf,
    pub truth: Vec<Vec<String>>,
}

/// 解析真值文本：一行一行，段落之间空一行。
pub fn parse_truth(text: &str) -> Vec<Vec<String>> {
    text.replace("\r\n", "\n")
        .split("\n\n")
        .map(|p| p.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).map(str::to_string).collect::<Vec<_>>())
        .filter(|p| !p.is_empty())
        .collect()
}

/// 读取目录里的样本：每个 `<名字>.png`（或 .jpg）配一个 `<名字>.txt`，按名字排序。
pub fn load_samples(dir: &Path) -> std::io::Result<Vec<Sample>> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let path = e?.path();
        let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
        if ext != "png" && ext != "jpg" {
            continue;
        }
        let txt = path.with_extension("txt");
        let Ok(text) = std::fs::read_to_string(&txt) else { continue };
        let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        out.push(Sample { name, image: path, truth: parse_truth(&text) });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn levenshtein_basics() {
        let c = |x: &str| x.chars().collect::<Vec<_>>();
        assert_eq!(levenshtein(&c("kitten"), &c("sitting")), 3);
        assert_eq!(levenshtein(&c(""), &c("abc")), 3);
        assert_eq!(levenshtein(&c("翻译"), &c("翻译")), 0);
        assert_eq!(normalize(" a b\tc… "), c("abc..."));
    }

    #[test]
    fn perfect_and_order_independent() {
        let truth = vec![s(&["Hello world", "second line"]), s(&["你好 世界"])];
        // 顺序不同、空格不同都不算错
        let pred = s(&["你好世界", "Helloworld", "second line"]);
        let c = evaluate(&truth, &pred, &[vec![1, 2], vec![0]]);
        assert_eq!((c.edits, c.recalled_lines, c.truth_lines), (0, 3, 3));
        assert_eq!((c.para_hits, c.truth_paras, c.pred_paras), (2, 2, 2));
        assert!((c.para_f1() - 1.0).abs() < 1e-9 && c.cer() == 0.0);
    }

    #[test]
    fn errors_missing_and_extra_lines() {
        let truth = vec![s(&["abcdefghij"]), s(&["missing"])];
        // 一个字错（10%，算召回）；漏一行 7 字；多一行 3 字
        let pred = s(&["abcdefghiX", "xyz"]);
        let c = evaluate(&truth, &pred, &[vec![0], vec![1]]);
        assert_eq!(c.edits, 1 + 7 + 3);
        assert_eq!(c.truth_chars, 17);
        assert_eq!(c.recalled_lines, 1);
        assert_eq!(c.para_hits, 1);
        assert!((c.para_f1() - 0.5).abs() < 1e-9);
        // 每行一段（还没有段落拼接）时，多行段落拿不到分
        let truth = vec![s(&["line one", "line two"])];
        let pred = s(&["line one", "line two"]);
        assert_eq!(evaluate(&truth, &pred, &[vec![0], vec![1]]).para_hits, 0);
        assert_eq!(evaluate(&truth, &pred, &[vec![0, 1]]).para_hits, 1);
    }

    #[test]
    fn truth_parsing() {
        assert_eq!(parse_truth("a\nb\n\n\nc\r\n"), vec![s(&["a", "b"]), s(&["c"])]);
    }
}
