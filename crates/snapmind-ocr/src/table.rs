//! 表格识别（需求 12.1 第 5 条、M8.2）：SLANet_plus 还原表格结构，再把本地 OCR 识别出的文字放进单元格。
//!
//! - 先裁掉四周的空白（模型是在裁好的表格上训练的，大片空白会让它把空白当成空行空列）。
//! - 模型输入：BGR，长边缩到 488，按 ImageNet 均值方差归一化，右下补零到 488×488。
//! - 模型输出：每一步一个结构标记（`<tr>` `<td></td>` `<td` ` colspan="2"` `>` …）的概率，以及每一步
//!   的单元格框（8 个数，四个角，按补零后的正方形归一化——横竖都乘长边）。贪心解码到 `eos`；遇到单元格
//!   标记就取这一步的框。
//! - 模型常漏掉合并单元格：一行文字横跨到右边的空单元格里（盖住它三成以上）时，把两格合成一格。
//! - 单元格文字：每个 OCR 行放进和它重叠面积最大的单元格（都不重叠时放进中心最近的），同一格里的行
//!   按段落规则拼接。

use std::path::Path;
use std::sync::Mutex;

use ort::session::Session;
use ort::value::Tensor;

use crate::imgops::sample_chw;
use crate::{layout, Image, Line, OcrError};

const SIDE: usize = 488;
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];

/// 结构标记表（和固定版本的 SLANet_plus 的 inference.yml 一致：去掉 `<td>`、末尾加 `<td></td>`，
/// 前后加 sos / eos）。
fn dictionary() -> Vec<String> {
    let mut d: Vec<String> = ["sos", "<thead>", "</thead>", "<tbody>", "</tbody>", "<tr>", "</tr>", "<td", ">", "</td>"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    d.extend((2..=20).map(|n| format!(" colspan=\"{n}\"")));
    d.extend((2..=20).map(|n| format!(" rowspan=\"{n}\"")));
    d.push("<td></td>".into());
    d.push("eos".into());
    d
}

/// 一个单元格。被合并的位置不单独出现（和 HTML 一样）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub text: String,
    pub colspan: u32,
    pub rowspan: u32,
    /// 在 `<thead>` 里（表头）
    pub header: bool,
}

/// 还原出的表格：一行一行，每行是这一行开始的单元格。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Table {
    pub rows: Vec<Vec<Cell>>,
}

/// 模型给出的结构：标记序列和每个单元格的框（原图像素，x0 y0 x1 y1，顺序和单元格标记一致）。
#[derive(Debug, Clone, Default)]
pub struct Structure {
    pub tokens: Vec<String>,
    pub cells: Vec<[f32; 4]>,
}

pub struct TableModel {
    session: Mutex<Session>,
    dict: Vec<String>,
}

impl TableModel {
    pub fn load(path: &Path) -> Result<Self, OcrError> {
        let env = crate::environment()?;
        let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).saturating_sub(2).clamp(2, 8);
        Ok(Self { session: Mutex::new(crate::session(env, path, threads)?), dict: dictionary() })
    }

    /// 识别表格结构。
    pub fn structure(&self, img: Image) -> Result<Structure, OcrError> {
        if img.width == 0 || img.height == 0 {
            return Err(OcrError::BadImage);
        }
        let (cx, cy, cw, ch) = content_bounds(img);
        let ratio = SIDE as f32 / cw.max(ch) as f32;
        let rw = ((cw as f32 * ratio) as usize).clamp(1, SIDE);
        let rh = ((ch as f32 * ratio) as usize).clamp(1, SIDE);
        // 先采样到 rh 行、行宽 SIDE 的缓冲区（右边留零），再按通道搬进 SIDE×SIDE（下边留零）
        let mut part = vec![0f32; 3 * rh * SIDE];
        sample_chw(img, (cx as f32, cy as f32, cw as f32, ch as f32), rw, rh, SIDE, MEAN, STD, &mut part);
        let mut input = vec![0f32; 3 * SIDE * SIDE];
        for c in 0..3 {
            input[c * SIDE * SIDE..c * SIDE * SIDE + rh * SIDE].copy_from_slice(&part[c * rh * SIDE..(c + 1) * rh * SIDE]);
        }
        let tensor = Tensor::from_array(([1usize, 3, SIDE, SIDE], input))?;
        let mut session = self.session.lock().expect("表格模型锁");
        let outputs = session.run(ort::inputs![tensor])?;
        // 两个输出：单元格框（最后一维 8）和结构概率（最后一维 = 标记数）
        let mut loc: Option<(Vec<i64>, Vec<f32>)> = None;
        let mut probs: Option<(Vec<i64>, Vec<f32>)> = None;
        for (_, v) in outputs.iter() {
            let (shape, data) = v.try_extract_tensor::<f32>()?;
            let dims: Vec<i64> = shape.iter().copied().collect();
            if dims.len() != 3 {
                continue;
            }
            if dims[2] == 8 {
                loc = Some((dims, data.to_vec()));
            } else if dims[2] as usize == self.dict.len() {
                probs = Some((dims, data.to_vec()));
            }
        }
        let (Some((ld, loc)), Some((pd, probs))) = (loc, probs) else {
            return Err(OcrError::BadOutput("表格模型的输出形状不对".into()));
        };
        let mut s = decode(&self.dict, &probs, pd[1] as usize, &loc, ld[1] as usize, cw.max(ch) as f32);
        for c in s.cells.iter_mut() {
            *c = [c[0] + cx as f32, c[1] + cy as f32, c[2] + cx as f32, c[3] + cy as f32];
        }
        Ok(s)
    }
}

/// 去掉四周和背景同色的空白，返回内容范围 (x, y, 宽, 高)，四周留 4 像素。背景取四条边像素的中位数。
pub fn content_bounds(img: Image) -> (u32, u32, u32, u32) {
    let (w, h) = (img.width, img.height);
    let px = |x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        [img.rgba[i], img.rgba[i + 1], img.rgba[i + 2]]
    };
    let mut border: [Vec<u8>; 3] = Default::default();
    for x in 0..w {
        for y in [0, h - 1] {
            let p = px(x, y);
            (0..3).for_each(|c| border[c].push(p[c]));
        }
    }
    for y in 0..h {
        for x in [0, w - 1] {
            let p = px(x, y);
            (0..3).for_each(|c| border[c].push(p[c]));
        }
    }
    let bg: Vec<u8> = border
        .iter_mut()
        .map(|v| {
            v.sort_unstable();
            v[v.len() / 2]
        })
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (w, h, 0, 0);
    for y in 0..h {
        for x in 0..w {
            let p = px(x, y);
            if (0..3).any(|c| p[c].abs_diff(bg[c]) > 24) {
                x0 = x0.min(x);
                y0 = y0.min(y);
                x1 = x1.max(x);
                y1 = y1.max(y);
            }
        }
    }
    if x1 < x0 || y1 < y0 {
        return (0, 0, w, h);
    }
    let pad = 4;
    let (x0, y0) = (x0.saturating_sub(pad), y0.saturating_sub(pad));
    let (x1, y1) = ((x1 + pad + 1).min(w), (y1 + pad + 1).min(h));
    (x0, y0, x1 - x0, y1 - y0)
}

/// 贪心解码（`probs` 是 [步数, 标记数]，`loc` 是 [步数, 8]）。框按补零后的正方形归一化，`side` 是它在原图里的边长。
fn decode(dict: &[String], probs: &[f32], steps: usize, loc: &[f32], loc_steps: usize, side: f32) -> Structure {
    let n = dict.len();
    let (sos, eos) = (0, n - 1);
    let mut out = Structure::default();
    for t in 0..steps {
        let row = &probs[t * n..(t + 1) * n];
        let idx = row.iter().enumerate().max_by(|a, b| a.1.total_cmp(b.1)).map(|(i, _)| i).unwrap_or(eos);
        if t > 0 && idx == eos {
            break;
        }
        if idx == sos || idx == eos {
            continue;
        }
        let tok = &dict[idx];
        if (tok == "<td" || tok == "<td></td>") && t < loc_steps {
            let b = &loc[t * 8..t * 8 + 8];
            let xs = [b[0], b[2], b[4], b[6]].map(|v| v * side);
            let ys = [b[1], b[3], b[5], b[7]].map(|v| v * side);
            let min = |a: [f32; 4]| a.iter().copied().fold(f32::MAX, f32::min);
            let max = |a: [f32; 4]| a.iter().copied().fold(f32::MIN, f32::max);
            out.cells.push([min(xs), min(ys), max(xs), max(ys)]);
        }
        out.tokens.push(tok.clone());
    }
    out
}

/// 把结构标记解析成行和单元格（文字为空）。
pub fn grid(tokens: &[String]) -> Vec<Vec<Cell>> {
    let mut rows: Vec<Vec<Cell>> = Vec::new();
    let mut header = false;
    let mut cur: Option<Cell> = None;
    let span = |t: &str, key: &str| -> Option<u32> { t.trim().strip_prefix(key)?.trim_matches(|c| c == '"' || c == '=').parse().ok() };
    for t in tokens {
        match t.as_str() {
            "<thead>" => header = true,
            "</thead>" => header = false,
            "<tr>" => rows.push(Vec::new()),
            "<td></td>" | "<td" => {
                if rows.is_empty() {
                    rows.push(Vec::new());
                }
                let c = Cell { text: String::new(), colspan: 1, rowspan: 1, header };
                if t == "<td></td>" {
                    rows.last_mut().unwrap().push(c);
                } else {
                    cur = Some(c);
                }
            }
            ">" => {
                if let (Some(c), Some(r)) = (cur.take(), rows.last_mut()) {
                    r.push(c);
                }
            }
            other => {
                if let Some(c) = cur.as_mut() {
                    if let Some(n) = span(other, "colspan") {
                        c.colspan = n.max(1);
                    } else if let Some(n) = span(other, "rowspan") {
                        c.rowspan = n.max(1);
                    }
                }
            }
        }
    }
    rows.retain(|r| !r.is_empty());
    rows
}

fn overlap(a: [f32; 4], b: [f32; 4]) -> f32 {
    let w = (a[2].min(b[2]) - a[0].max(b[0])).max(0.0);
    let h = (a[3].min(b[3]) - a[1].max(b[1])).max(0.0);
    w * h
}

/// 结构 + OCR 行 → 表格。行的坐标和结构的坐标都是同一张图的像素。
pub fn assemble(s: &Structure, lines: &[Line]) -> Table {
    let mut rows = grid(&s.tokens);
    let total: usize = rows.iter().map(Vec::len).sum();
    let n = total.min(s.cells.len());
    // 每个单元格分到的行
    let mut owned: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (li, l) in lines.iter().enumerate() {
        let k = &l.ink;
        let lb = [k.left as f32, k.top as f32, k.right() as f32, k.bottom() as f32];
        let center = ((lb[0] + lb[2]) / 2.0, (lb[1] + lb[3]) / 2.0);
        let best = (0..n)
            .map(|ci| (ci, overlap(lb, s.cells[ci])))
            .filter(|&(_, a)| a > 0.0)
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(ci, _)| ci)
            .or_else(|| {
                (0..n).min_by(|&a, &b| {
                    let d = |c: [f32; 4]| ((c[0] + c[2]) / 2.0 - center.0).powi(2) + ((c[1] + c[3]) / 2.0 - center.1).powi(2);
                    d(s.cells[a]).total_cmp(&d(s.cells[b]))
                })
            });
        if let Some(ci) = best {
            owned[ci].push(li);
        }
    }
    let mut ci = 0;
    let mut boxes: Vec<Vec<Option<[f32; 4]>>> = Vec::new();
    for row in rows.iter_mut() {
        let mut rb = Vec::new();
        for cell in row.iter_mut() {
            if ci < n {
                cell.text = layout::compose(lines, &owned[ci], false).replace("\n\n", "\n");
                rb.push(Some(s.cells[ci]));
            } else {
                rb.push(None);
            }
            ci += 1;
        }
        boxes.push(rb);
    }
    // 漏掉的合并单元格：一格里的文字横跨到相邻的空单元格（盖住它三成以上）就把两格合并。
    // 文字可能被分到左边那格（伸进右边），也可能被分到右边那格（伸进左边）。
    ci = 0;
    for (r, row) in rows.iter_mut().enumerate() {
        // 这一行每格里文字的左右范围
        let mut extent: Vec<(f32, f32)> = Vec::new();
        for _ in row.iter() {
            let e = if ci < n {
                let ls = owned[ci].iter().map(|&li| &lines[li].ink);
                ls.fold((f32::MAX, f32::MIN), |(a, b), k| (a.min(k.left as f32), b.max(k.right() as f32)))
            } else {
                (f32::MAX, f32::MIN)
            };
            extent.push(e);
            ci += 1;
        }
        let mut i = 0;
        while i + 1 < row.len() {
            let (Some(lb), Some(rb)) = (boxes[r][i], boxes[r][i + 1]) else {
                i += 1;
                continue;
            };
            let (le, re) = (row[i].text.is_empty(), row[i + 1].text.is_empty());
            // 左格有字、右格空：左格文字的右边伸进右格多少；左格空、右格有字：右格文字的左边伸进左格多少
            let into_right = !le && re && (extent[i].1 - rb[0]) / (rb[2] - rb[0]).max(1.0) >= 0.3;
            let into_left = le && !re && (lb[2] - extent[i + 1].0) / (lb[2] - lb[0]).max(1.0) >= 0.3;
            if into_right || into_left {
                let span = row[i].colspan + row[i + 1].colspan;
                if into_left {
                    row[i].text = std::mem::take(&mut row[i + 1].text);
                    extent[i] = extent[i + 1];
                }
                row[i].colspan = span;
                row.remove(i + 1);
                boxes[r][i] = Some([lb[0], lb[1].min(rb[1]), rb[2], lb[3].max(rb[3])]);
                boxes[r].remove(i + 1);
                extent.remove(i + 1);
            } else {
                i += 1;
            }
        }
    }
    Table { rows }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Ink;

    fn toks(s: &[&str]) -> Vec<String> {
        s.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn dictionary_matches_model_layout() {
        let d = dictionary();
        assert_eq!(d.len(), 50);
        assert_eq!((d[0].as_str(), d[7].as_str(), d[48].as_str(), d[49].as_str()), ("sos", "<td", "<td></td>", "eos"));
    }

    #[test]
    fn parses_spans_and_header() {
        let t = toks(&[
            "<thead>", "<tr>", "<td></td>", "<td></td>", "</tr>", "</thead>", "<tbody>", "<tr>", "<td", " colspan=\"2\"", ">", "</td>",
            "</tr>", "<tr>", "<td", " rowspan=\"3\"", ">", "</td>", "<td></td>", "</tr>", "</tbody>",
        ]);
        let g = grid(&t);
        assert_eq!(g.len(), 3);
        assert!(g[0].iter().all(|c| c.header));
        assert_eq!((g[1][0].colspan, g[1][0].header), (2, false));
        assert_eq!((g[2][0].rowspan, g[2].len()), (3, 2));
    }

    #[test]
    fn decodes_until_eos_with_boxes() {
        let d = dictionary();
        let n = d.len();
        let mut probs = vec![0f32; 4 * n];
        for (t, idx) in [5usize, 48, 49, 6].iter().enumerate() {
            probs[t * n + idx] = 1.0; // <tr> <td></td> eos </tr>（eos 之后的不要）
        }
        let mut loc = vec![0f32; 4 * 8];
        loc[8..16].copy_from_slice(&[0.1, 0.2, 0.5, 0.2, 0.5, 0.6, 0.1, 0.6]);
        let s = decode(&d, &probs, 4, &loc, 4, 100.0);
        assert_eq!(s.tokens, toks(&["<tr>", "<td></td>"]));
        assert_eq!(s.cells.len(), 1);
        for (a, b) in s.cells[0].iter().zip([10.0, 20.0, 50.0, 60.0]) {
            assert!((a - b).abs() < 1e-3, "{:?}", s.cells);
        }
    }

    #[test]
    fn assigns_lines_to_overlapping_cells() {
        let s = Structure {
            tokens: toks(&["<tr>", "<td></td>", "<td></td>", "</tr>"]),
            cells: vec![[0.0, 0.0, 100.0, 40.0], [100.0, 0.0, 200.0, 40.0]],
        };
        let line = |t: &str, x: u32| Line {
            text: t.into(),
            x,
            y: 10,
            width: 60,
            height: 16,
            score: 1.0,
            ink: Ink { left: x, top: 10, width: 60, height: 16, color: [0; 3], background: [255; 3] },
        };
        let t = assemble(&s, &[line("右边", 120), line("左边", 10)]);
        assert_eq!(t.rows[0][0].text, "左边");
        assert_eq!(t.rows[0][1].text, "右边");

        // 模型漏掉合并单元格：一行字横跨三格里的后两格，被分到右边那格、左边空着 → 合成一格
        let s = Structure {
            tokens: toks(&["<tr>", "<td></td>", "<td></td>", "<td></td>", "</tr>"]),
            cells: vec![[0.0, 0.0, 100.0, 40.0], [100.0, 0.0, 160.0, 40.0], [160.0, 0.0, 300.0, 40.0]],
        };
        let mut wide = line("识别为 Markdown", 108);
        wide.ink.width = 140; // 108..248：盖住第三格大半，伸进第二格 52/60
        let t = assemble(&s, &[line("Shift+Alt+R", 5), wide]);
        assert_eq!(t.rows[0].len(), 2);
        assert_eq!((t.rows[0][1].text.as_str(), t.rows[0][1].colspan), ("识别为 Markdown", 2));
    }
}
