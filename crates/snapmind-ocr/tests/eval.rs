//! OCR 回归测试：在 tests/fixtures/ocr-samples 上跑评测，指标低于阈值就失败。
//! 阈值略低于当前实测值（见 docs/进度.md 的 OCR 评测记录），每次改进后上调。
//! 模型不在仓库里，没有时跳过。

use std::path::Path;

use snapmind_ocr::eval::{self, Counts};
use snapmind_ocr::{layout, Image, Ocr};

/// 字符错误率上限（当前 0.97%）
const MAX_CER: f64 = 0.015;
/// 行召回率下限（当前 96.0%）
const MIN_RECALL: f64 = 0.94;
/// 段落 F1 下限（M4.3 段落拼接前每行单独成段，当前 0.543；M4.3 的验收是 ≥ 0.9）
const MIN_PARA_F1: f64 = 0.5;

#[test]
fn sample_set_does_not_regress() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let models = root.join("../../models/ppocrv5");
    if !models.join("rec.onnx").exists() {
        eprintln!("跳过：没有 OCR 模型（先运行 node scripts/fetch-models.mjs）");
        return;
    }
    let samples = eval::load_samples(&root.join("tests/fixtures/ocr-samples")).unwrap();
    assert!(samples.len() >= 12, "样本集不完整");
    let ocr = Ocr::load(&models).unwrap();
    let mut total = Counts::default();
    for s in &samples {
        let img = image::open(&s.image).unwrap().to_rgba8();
        let (lines, _) = ocr.recognize(Image { width: img.width(), height: img.height(), rgba: img.as_raw() }).unwrap();
        let texts: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
        let c = eval::evaluate(&s.truth, &texts, &layout::paragraphs(&lines));
        eprintln!("{:<18} CER {:>6.2}%  召回 {:>6.2}%  段落 F1 {:.3}", s.name, c.cer() * 100.0, c.line_recall() * 100.0, c.para_f1());
        total.add(&c);
    }
    eprintln!("合计 CER {:.2}%  召回 {:.2}%  段落 F1 {:.3}", total.cer() * 100.0, total.line_recall() * 100.0, total.para_f1());
    assert!(total.cer() <= MAX_CER, "字符错误率 {:.2}% 超过 {:.2}%", total.cer() * 100.0, MAX_CER * 100.0);
    assert!(total.line_recall() >= MIN_RECALL, "行召回率 {:.2}% 低于 {:.2}%", total.line_recall() * 100.0, MIN_RECALL * 100.0);
    assert!(total.para_f1() >= MIN_PARA_F1, "段落 F1 {:.3} 低于 {MIN_PARA_F1}", total.para_f1());
}
