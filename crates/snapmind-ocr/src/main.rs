//! 命令行：`snapmind-ocr run <图片> [--models <目录>] [--repeat N]`
//!
//! 打印识别出的每一行（坐标、置信度、文字）和各阶段耗时。M4.2 在这里加 `eval`。

use std::path::PathBuf;
use std::process::ExitCode;

use snapmind_ocr::{Image, Ocr};

fn default_models() -> PathBuf {
    // 从仓库根目录或 crate 目录运行都能找到
    for p in ["models/ppocrv5", "../../models/ppocrv5"] {
        let p = PathBuf::from(p);
        if p.join("det.onnx").exists() {
            return p;
        }
    }
    PathBuf::from("models/ppocrv5")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str| args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned();
    match args.first().map(String::as_str) {
        Some("run") => {}
        _ => {
            eprintln!("用法：snapmind-ocr run <图片> [--models <目录>] [--repeat N]");
            return ExitCode::from(2);
        }
    }
    let Some(path) = args.get(1) else {
        eprintln!("缺少图片路径");
        return ExitCode::from(2);
    };
    let models = flag("--models").map(PathBuf::from).unwrap_or_else(default_models);
    let repeat: usize = flag("--repeat").and_then(|s| s.parse().ok()).unwrap_or(1).max(1);

    let img = match image::open(path) {
        Ok(i) => i.to_rgba8(),
        Err(e) => {
            eprintln!("读取图片失败：{e}");
            return ExitCode::FAILURE;
        }
    };
    let t = std::time::Instant::now();
    let ocr = match Ocr::load(&models) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}（先运行 node scripts/fetch-models.mjs）");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("加载模型 {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let mut ocr = ocr;
    if let Some(side) = flag("--det-side").and_then(|s| s.parse().ok()) {
        ocr.det_options.max_side = side;
    }
    let image = Image { width: img.width(), height: img.height(), rgba: img.as_raw() };
    let mut last = None;
    for i in 0..repeat {
        match ocr.recognize(image) {
            Ok((lines, timing)) => {
                eprintln!(
                    "第 {} 次：{}×{}，{} 行，检测 {:.0} ms，识别 {:.0} ms，共 {:.0} ms",
                    i + 1,
                    image.width,
                    image.height,
                    lines.len(),
                    timing.det.as_secs_f64() * 1e3,
                    timing.rec.as_secs_f64() * 1e3,
                    timing.total.as_secs_f64() * 1e3
                );
                last = Some(lines);
            }
            Err(e) => {
                eprintln!("识别失败：{e}");
                return ExitCode::FAILURE;
            }
        }
    }
    for l in last.unwrap_or_default() {
        println!("{:>5},{:<5} {:>4}×{:<3} {:.2}  {}", l.x, l.y, l.width, l.height, l.score, l.text);
    }
    ExitCode::SUCCESS
}
