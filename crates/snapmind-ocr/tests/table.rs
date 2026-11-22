//! 表格识别端到端：用真实的 SLANet_plus + PP-OCRv5 识别 tests/fixtures/table.png（table.html 渲染）。
//! 模型不在仓库里（node scripts/fetch-models.mjs 下载），没有时跳过。

use std::path::Path;

use snapmind_ocr::table::{assemble, TableModel};
use snapmind_ocr::{Image, Ocr};

#[test]
fn recognizes_fixture_table() {
    let models = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/ppocrv5");
    if !models.join("table.onnx").exists() || !models.join("rec.onnx").exists() {
        eprintln!("跳过：没有表格 / OCR 模型（先运行 node scripts/fetch-models.mjs）");
        return;
    }
    let img = image::open(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/table.png")).unwrap().to_rgba8();
    let image = Image { width: img.width(), height: img.height(), rgba: img.as_raw() };
    let model = TableModel::load(&models.join("table.onnx")).unwrap();
    let t0 = std::time::Instant::now();
    let s = model.structure(image).unwrap();
    let t_struct = t0.elapsed();
    let (lines, _) = Ocr::load(&models).unwrap().recognize(image).unwrap();
    let table = assemble(&s, &lines);
    let texts: Vec<Vec<String>> = table.rows.iter().map(|r| r.iter().map(|c| c.text.replace(' ', "")).collect()).collect();
    eprintln!("结构 {:.0} ms：{texts:?}", t_struct.as_secs_f64() * 1e3);
    assert_eq!(texts.len(), 5, "5 行");
    assert_eq!(texts[0], vec!["快捷键", "动作", "说明"]);
    assert_eq!(texts[1], vec!["Shift+F1", "截图", "进入编辑态"]);
    assert_eq!(texts[2], vec!["Ctrl+Q", "翻译", "识别后翻译选区"]);
    // 第 4 行第二格横跨两列
    assert_eq!(texts[3], vec!["Shift+Alt+R", "识别为Markdown"]);
    assert_eq!(table.rows[3][1].colspan, 2);
    assert_eq!(texts[4], vec!["Ctrl+Alt+D", "划词", "1.2s"]);
}
