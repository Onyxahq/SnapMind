//! 端到端：用真实模型识别 tests/fixtures/lines.png，逐行与 lines.txt 对比。
//! 模型不在仓库里（node scripts/fetch-models.mjs 下载），没有时跳过。

use std::path::Path;

use snapmind_ocr::{Image, Ocr};

#[test]
fn recognizes_fixture_lines() {
    let models = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../models/ppocrv5");
    if !models.join("rec.onnx").exists() {
        eprintln!("跳过：没有 OCR 模型（先运行 node scripts/fetch-models.mjs）");
        return;
    }
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let img = image::open(fixtures.join("lines.png")).unwrap().to_rgba8();
    let expected: Vec<String> = std::fs::read_to_string(fixtures.join("lines.txt"))
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.replace(' ', ""))
        .collect();
    let ocr = Ocr::load(&models).unwrap();
    let (lines, timing) = ocr.recognize(Image { width: img.width(), height: img.height(), rgba: img.as_raw() }).unwrap();
    let got: Vec<String> = lines.iter().map(|l| l.text.replace(' ', "")).collect();
    // 模型会省略中英文之间的空格（段落拼接时再处理），这里比较时忽略空格
    assert_eq!(got, expected);
    assert!(lines.iter().all(|l| l.score > 0.9), "{lines:?}");
    // 行框在图里、从上到下
    assert!(lines.windows(2).all(|w| w[0].y < w[1].y));
    eprintln!("fixture：{} 行，{:?}", lines.len(), timing);
}
