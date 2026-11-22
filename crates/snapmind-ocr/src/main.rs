//! 命令行：
//! - `snapmind-ocr run <图片> [--models <目录>] [--repeat N] [--det-side N]`：打印识别出的行（含笔画范围和颜色）、段落和耗时
//! - `snapmind-ocr eval [样本目录] [--models <目录>] [--out 报告.md] [--max-cer X] [--min-recall X] [--min-para-f1 X] [--verbose]`：
//!   在样本集上算字符错误率、行召回率、段落 F1 和耗时，低于阈值时退出码为 1（用于回归测试）。

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use snapmind_ocr::eval::{self, Counts};
use snapmind_ocr::{layout, Image, Ocr};

/// 从仓库根目录或 crate 目录运行都能找到。
fn find(rel_from_root: &str) -> PathBuf {
    for base in ["", "../../"] {
        let p = PathBuf::from(format!("{base}{rel_from_root}"));
        if p.exists() {
            return p;
        }
    }
    PathBuf::from(rel_from_root)
}

struct Args(Vec<String>);

impl Args {
    fn flag(&self, name: &str) -> Option<&str> {
        self.0.iter().position(|a| a == name).and_then(|i| self.0.get(i + 1)).map(String::as_str)
    }
    fn has(&self, name: &str) -> bool {
        self.0.iter().any(|a| a == name)
    }
    fn num(&self, name: &str) -> Option<f64> {
        self.flag(name).and_then(|s| s.parse().ok())
    }
    /// 第 n 个不以 -- 开头、也不是某个选项的值的参数
    fn positional(&self, n: usize) -> Option<&str> {
        let mut skip = false;
        self.0
            .iter()
            .filter(|a| {
                if skip {
                    skip = false;
                    return false;
                }
                if a.starts_with("--") {
                    skip = !matches!(a.as_str(), "--verbose");
                    return false;
                }
                true
            })
            .nth(n)
            .map(String::as_str)
    }
}

fn load(args: &Args) -> Result<Ocr, ExitCode> {
    let models = args.flag("--models").map(PathBuf::from).unwrap_or_else(|| find("models/ppocrv5"));
    let t = Instant::now();
    let mut ocr = Ocr::load(&models).map_err(|e| {
        eprintln!("{e}（先运行 node scripts/fetch-models.mjs）");
        ExitCode::FAILURE
    })?;
    if let Some(side) = args.num("--det-side") {
        ocr.det_options.max_side = side as u32;
    }
    eprintln!("加载模型 {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    Ok(ocr)
}

fn open(path: &Path) -> Result<image::RgbaImage, ExitCode> {
    image::open(path).map(|i| i.to_rgba8()).map_err(|e| {
        eprintln!("读取 {} 失败：{e}", path.display());
        ExitCode::FAILURE
    })
}

fn run(args: &Args) -> Result<(), ExitCode> {
    let path = args.positional(1).ok_or_else(|| {
        eprintln!("缺少图片路径");
        ExitCode::from(2)
    })?;
    let img = open(Path::new(path))?;
    let ocr = load(args)?;
    let repeat = args.num("--repeat").unwrap_or(1.0).max(1.0) as usize;
    let image = Image { width: img.width(), height: img.height(), rgba: img.as_raw() };
    let mut last = Vec::new();
    for i in 0..repeat {
        let (lines, t) = ocr.recognize(image).map_err(|e| {
            eprintln!("识别失败：{e}");
            ExitCode::FAILURE
        })?;
        eprintln!(
            "第 {} 次：{}×{}，{} 行，检测 {:.0} ms，识别 {:.0} ms，共 {:.0} ms",
            i + 1,
            image.width,
            image.height,
            lines.len(),
            t.det.as_secs_f64() * 1e3,
            t.rec.as_secs_f64() * 1e3,
            t.total.as_secs_f64() * 1e3
        );
        last = lines;
    }
    let hex = |c: [u8; 3]| format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]);
    for l in &last {
        let k = &l.ink;
        println!(
            "{:>5},{:<5} {:>4}×{:<3} 笔画 {:>4},{:<4} {:>4}×{:<3} {}/{} {:.2}  {}",
            l.x, l.y, l.width, l.height, k.left, k.top, k.width, k.height, hex(k.color), hex(k.background), l.score, l.text
        );
    }
    println!("--- 段落");
    for p in layout::paragraphs(&last) {
        println!("{}\n", layout::paragraph_text(&last, &p));
    }
    Ok(())
}

fn pct(x: f64) -> String {
    format!("{:.2}%", x * 100.0)
}

fn eval_cmd(args: &Args) -> Result<(), ExitCode> {
    let dir = args.positional(1).map(PathBuf::from).unwrap_or_else(|| find("crates/snapmind-ocr/tests/fixtures/ocr-samples"));
    let samples = eval::load_samples(&dir).map_err(|e| {
        eprintln!("读取样本目录 {} 失败：{e}", dir.display());
        ExitCode::FAILURE
    })?;
    if samples.is_empty() {
        eprintln!("{} 里没有样本（<名字>.png + <名字>.txt）", dir.display());
        return Err(ExitCode::FAILURE);
    }
    let ocr = load(args)?;
    let mut report = String::from(
        "| 样本 | 尺寸 | 字数 | 字符错误率 | 行召回率 | 段落 F1 | 行（真值 / 识别） | 耗时 |\n|---|---|---|---|---|---|---|---|\n",
    );
    let mut total = Counts::default();
    let mut total_ms = 0.0;
    for s in &samples {
        let img = open(&s.image)?;
        let image = Image { width: img.width(), height: img.height(), rgba: img.as_raw() };
        let (lines, t) = ocr.recognize(image).map_err(|e| {
            eprintln!("{}：识别失败：{e}", s.name);
            ExitCode::FAILURE
        })?;
        let texts: Vec<String> = lines.iter().map(|l| l.text.clone()).collect();
        let paras = layout::paragraphs(&lines);
        let c = eval::evaluate(&s.truth, &texts, &paras);
        let ms = t.total.as_secs_f64() * 1e3;
        total_ms += ms;
        total.add(&c);
        report.push_str(&format!(
            "| {} | {}×{} | {} | {} | {} | {:.3} | {} / {} | {:.0} ms |\n",
            s.name,
            image.width,
            image.height,
            c.truth_chars,
            pct(c.cer()),
            pct(c.line_recall()),
            c.para_f1(),
            c.truth_lines,
            c.pred_lines,
            ms
        ));
        if args.has("--verbose") {
            let truth: Vec<&String> = s.truth.iter().flatten().collect();
            for l in &texts {
                if !truth.iter().any(|t| eval::normalize(t) == eval::normalize(l)) {
                    eprintln!("  [{}] 识别：{l}", s.name);
                }
            }
            for t in truth {
                if !texts.iter().any(|l| eval::normalize(t) == eval::normalize(l)) {
                    eprintln!("  [{}] 真值：{t}", s.name);
                }
            }
        }
    }
    report.push_str(&format!(
        "| **合计** | {} 个样本 | {} | **{}** | **{}** | **{:.3}** | {} / {} | 平均 {:.0} ms |\n",
        samples.len(),
        total.truth_chars,
        pct(total.cer()),
        pct(total.line_recall()),
        total.para_f1(),
        total.truth_lines,
        total.pred_lines,
        total_ms / samples.len() as f64
    ));
    println!("{report}");
    if let Some(out) = args.flag("--out") {
        if let Err(e) = std::fs::write(out, &report) {
            eprintln!("写报告失败：{e}");
        }
    }
    let mut ok = true;
    let mut check = |name: &str, bad: bool, msg: String| {
        if bad {
            eprintln!("✗ {name}：{msg}");
            ok = false;
        }
    };
    if let Some(m) = args.num("--max-cer") {
        check("字符错误率", total.cer() > m, format!("{} > {}", pct(total.cer()), pct(m)));
    }
    if let Some(m) = args.num("--min-recall") {
        check("行召回率", total.line_recall() < m, format!("{} < {}", pct(total.line_recall()), pct(m)));
    }
    if let Some(m) = args.num("--min-para-f1") {
        check("段落 F1", total.para_f1() < m, format!("{:.3} < {m:.3}", total.para_f1()));
    }
    if ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn main() -> ExitCode {
    let args = Args(std::env::args().skip(1).collect());
    let r = match args.positional(0) {
        Some("run") => run(&args),
        Some("eval") => eval_cmd(&args),
        _ => {
            eprintln!("用法：\n  snapmind-ocr run <图片> [--models <目录>] [--repeat N] [--det-side N]\n  snapmind-ocr eval [样本目录] [--models <目录>] [--out 报告.md] [--max-cer X] [--min-recall X] [--min-para-f1 X] [--verbose]");
            Err(ExitCode::from(2))
        }
    };
    r.err().unwrap_or(ExitCode::SUCCESS)
}
