//! 表格导出 .xlsx（需求 3.8「表格」、M8.2）：单元格按 HTML 表格的规则摆放（合并单元格占住的位置跳过），
//! 合并单元格写成 Excel 的合并区域，表头加粗，列宽按内容估算（中日韩文字算两个字宽）。

use snapmind_core::i18n::{trf};
use rust_xlsxwriter::{Format, Workbook};
use serde::Deserialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

use crate::state::AppState;

#[derive(Debug, Clone, Deserialize)]
pub struct CellIn {
    pub text: String,
    pub colspan: u32,
    pub rowspan: u32,
    #[serde(default)]
    pub header: bool,
}

/// 摆好位置的单元格：（行，列，单元格）。
pub fn place(rows: &[Vec<CellIn>]) -> Vec<(u32, u32, &CellIn)> {
    let mut taken: std::collections::HashSet<(u32, u32)> = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let r = r as u32;
        let mut c = 0u32;
        for cell in row {
            while taken.contains(&(r, c)) {
                c += 1;
            }
            let (cs, rs) = (cell.colspan.max(1), cell.rowspan.max(1));
            for dr in 0..rs {
                for dc in 0..cs {
                    taken.insert((r + dr, c + dc));
                }
            }
            out.push((r, c, cell));
            c += cs;
        }
    }
    out
}

/// 能按数字写的文字：「1,200」「-3.5」可以；「0012」（编号）、超过 15 位（证件号、长单号，Excel 会丢精度）
/// 保持文字。
pub fn as_number(text: &str) -> Option<f64> {
    let t = text.trim().replace(',', "");
    let digits = t.chars().filter(char::is_ascii_digit).count();
    let leading_zero = t.trim_start_matches('-').starts_with('0') && t.len() > 1 && !t.trim_start_matches('-').starts_with("0.");
    if t.is_empty() || digits > 15 || leading_zero {
        return None;
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

fn display_width(s: &str) -> f64 {
    s.lines()
        .map(|l| l.chars().map(|ch| if (ch as u32) >= 0x2E80 { 2.0 } else { 1.0 }).sum::<f64>())
        .fold(0.0, f64::max)
}

/// 写成 .xlsx 的字节。
pub fn build(rows: &[Vec<CellIn>]) -> Result<Vec<u8>, String> {
    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    let body = Format::new().set_text_wrap();
    let head = Format::new().set_bold().set_text_wrap();
    let mut widths: std::collections::HashMap<u32, f64> = std::collections::HashMap::new();
    let e = |x: rust_xlsxwriter::XlsxError| x.to_string();
    for (r, c, cell) in place(rows) {
        let fmt = if cell.header { &head } else { &body };
        let (cs, rs) = (cell.colspan.max(1), cell.rowspan.max(1));
        // 数字按数字写（Excel 里能直接求和），其余按文字
        let number = as_number(&cell.text);
        if cs > 1 || rs > 1 {
            ws.merge_range(r, c as u16, r + rs - 1, (c + cs - 1) as u16, &cell.text, fmt).map_err(e)?;
        } else if let Some(n) = number {
            ws.write_number_with_format(r, c as u16, n, fmt).map_err(e)?;
        } else {
            ws.write_string_with_format(r, c as u16, &cell.text, fmt).map_err(e)?;
        }
        if cs == 1 {
            let w = widths.entry(c).or_insert(8.0);
            *w = w.max(display_width(&cell.text) + 2.0).min(60.0);
        }
    }
    for (c, w) in widths {
        ws.set_column_width(c as u16, w).map_err(e)?;
    }
    wb.save_to_buffer().map_err(e)
}

/// 导出 .xlsx：系统另存为对话框（默认目录和文件名模板与截图一致）。返回保存路径，取消为 None。
#[tauri::command]
pub async fn save_xlsx(app: AppHandle, window: tauri::WebviewWindow, rows: Vec<Vec<CellIn>>) -> Result<Option<String>, String> {
    let bytes = build(&rows)?;
    let config = app.state::<AppState>().config.get();
    let dir = crate::export::default_dir(&app, &config);
    let name = snapmind_core::export::render_filename(&config.save.filename_template, snapmind_core::export::now_local());
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let _ = std::fs::create_dir_all(&dir);
        window
            .dialog()
            .file()
            .set_parent(&window)
            .set_directory(dir)
            .set_file_name(format!("{name}.xlsx"))
            .add_filter("Excel", &["xlsx"])
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = picked else { return Ok(None) };
    std::fs::write(&path, bytes).map_err(|e| trf("保存失败：{e}", &[("e", &e.to_string())]))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell(t: &str, cs: u32, rs: u32) -> CellIn {
        CellIn { text: t.into(), colspan: cs, rowspan: rs, header: false }
    }

    #[test]
    fn places_cells_around_spans() {
        // a 横跨两列；b 纵跨两行；第二行的 c 要跳过 b 占住的位置
        let rows = vec![vec![cell("a", 2, 1), cell("b", 1, 2)], vec![cell("c", 1, 1), cell("d", 1, 1)]];
        let p: Vec<(u32, u32, &str)> = place(&rows).into_iter().map(|(r, c, x)| (r, c, x.text.as_str())).collect();
        assert_eq!(p, vec![(0, 0, "a"), (0, 2, "b"), (1, 0, "c"), (1, 1, "d")]);
    }

    #[test]
    fn builds_a_valid_xlsx() {
        let rows = vec![vec![cell("名称", 1, 1), cell("数量", 1, 1)], vec![cell("苹果", 1, 1), cell("1,200", 1, 1)], vec![cell("合计", 2, 1)]];
        let bytes = build(&rows).unwrap();
        assert_eq!(&bytes[..2], b"PK", "xlsx 是 zip");
        assert!(bytes.len() > 1000);
        assert_eq!(display_width("中文ab"), 6.0);
        assert_eq!(as_number("1,200"), Some(1200.0));
        assert_eq!(as_number(" -3.5 "), Some(-3.5));
        assert_eq!(as_number("0.25"), Some(0.25));
        assert_eq!(as_number("0012"), None);
        assert_eq!(as_number("110101199003071234"), None);
        assert_eq!(as_number("1.2 s"), None);
        assert_eq!(as_number("inf"), None);
    }
}
