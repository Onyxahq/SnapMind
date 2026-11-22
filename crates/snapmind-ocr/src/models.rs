//! 模型文件：下载地址（固定到 Hugging Face 仓库的提交）、SHA-256、大小，以及从识别模型的
//! inference.yml 取出字符表。和 `scripts/fetch-models.mjs` 用同一份地址和哈希。
//!
//! 下载本身由应用做（它有网络和进度显示），这里只提供清单和校验后的整理。

use std::path::Path;

pub struct ModelFile {
    /// 存到模型目录里的文件名
    pub name: &'static str,
    pub url: &'static str,
    pub sha256: &'static str,
    pub size: u64,
}

macro_rules! det {
    ($f:literal) => {
        concat!("https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_det_onnx/resolve/e6f4fa85f00e168c862bc462aebca69eef9b3d3d/", $f)
    };
}
macro_rules! table {
    ($f:literal) => {
        concat!("https://huggingface.co/PaddlePaddle/SLANet_plus_onnx/resolve/7dbe640e127602bf506815e822c09758de73c482/", $f)
    };
}
macro_rules! rec {
    ($f:literal) => {
        concat!("https://huggingface.co/PaddlePaddle/PP-OCRv5_mobile_rec_onnx/resolve/ed152b8b495f84de93cda5709d768548a9127622/", $f)
    };
}

/// PP-OCRv5 mobile 检测 + 识别（PaddlePaddle 官方 ONNX，Apache-2.0）。
pub const FILES: [ModelFile; 4] = [
    ModelFile {
        name: "det.onnx",
        url: det!("inference.onnx"),
        sha256: "a431985659dc921974177a95adcfbb90fd9e51989a5e04d70d0b75f597b6e61d",
        size: 4_826_518,
    },
    ModelFile {
        name: "det.yml",
        url: det!("inference.yml"),
        sha256: "98069072e1b6b37d727fd9d9f11725faa46d6ea0de012f2ed26caea011c37699",
        size: 903,
    },
    ModelFile {
        name: "rec.onnx",
        url: rec!("inference.onnx"),
        sha256: "da72dc72ca4dc220df0dfde68c1dedc31c58d3e76a25871122e5056227d50092",
        size: 16_534_782,
    },
    ModelFile {
        name: "rec.yml",
        url: rec!("inference.yml"),
        sha256: "5dfeb2777f6d0db8177d8128a8acfcf6e6276dc4ac73ea3bf0dc06d6a5e85d8e",
        size: 148_345,
    },
];

/// 表格结构识别（M8.2）：SLANet_plus，和识别模型放在同一个目录。结构标记表固定在代码里（和这个版本一致）。
pub const TABLE_FILES: [ModelFile; 1] = [ModelFile {
    name: "table.onnx",
    url: table!("inference.onnx"),
    sha256: "7790c0c13ce064782c9d22ebeb16b4da8216f83d3ba576da962c106ef58386da",
    size: 7_782_138,
}];

/// 下载总大小（字节）。
pub fn total_size() -> u64 {
    FILES.iter().map(|f| f.size).sum()
}

/// 表格模型装好了（识别模型另外判断）。
pub fn table_installed(dir: &Path) -> bool {
    dir.join("table.onnx").is_file()
}

/// 模型目录里引擎要用的三个文件都在。
pub fn installed(dir: &Path) -> bool {
    ["det.onnx", "rec.onnx", "keys.txt"].iter().all(|f| dir.join(f).is_file())
}

/// 从识别模型的 inference.yml 里取出字符表（PostProcess.character_dict），每项一个字符。
pub fn extract_keys(yml: &str) -> Result<Vec<String>, String> {
    let lines: Vec<&str> = yml.lines().collect();
    let start = lines.iter().position(|l| l.trim() == "character_dict:").ok_or("rec.yml 里没有 character_dict")?;
    let first = lines.get(start + 1).ok_or("character_dict 是空的")?;
    let indent = &first[..first.len() - first.trim_start().len()];
    let prefix = format!("{indent}- ");
    let mut keys = Vec::new();
    for l in &lines[start + 1..] {
        let Some(v) = l.strip_prefix(prefix.as_str()) else { break };
        keys.push(unquote(v));
    }
    if keys.is_empty() {
        return Err("character_dict 是空的".into());
    }
    Ok(keys)
}

/// YAML 标量：'x'（'' 表示 '）、"x"（反斜杠转义）或裸值。
fn unquote(v: &str) -> String {
    if v.len() >= 2 && v.starts_with('\'') && v.ends_with('\'') {
        return v[1..v.len() - 1].replace("''", "'");
    }
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        let mut out = String::new();
        let mut it = v[1..v.len() - 1].chars();
        while let Some(c) = it.next() {
            if c != '\\' {
                out.push(c);
                continue;
            }
            match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('u') => {
                    let hex: String = it.by_ref().take(4).collect();
                    if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                }
                Some(other) => out.push(other),
                None => {}
            }
        }
        return out;
    }
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_from_yaml() {
        let yml = "PostProcess:\n  name: CTCLabelDecode\n  character_dict:\n  - 'a'\n  - ''''\n  - \"\\u4E2D\"\n  - 中\n  - ' '\nOther: 1\n";
        assert_eq!(extract_keys(yml).unwrap(), vec!["a", "'", "中", "中", " "]);
        assert!(extract_keys("x: 1").is_err());
    }

    #[test]
    fn urls_and_sizes() {
        assert!(FILES[0].url.contains("det_onnx") && FILES[0].url.ends_with("inference.onnx"));
        assert!(FILES[1].url.contains("det_onnx") && FILES[1].url.ends_with("inference.yml"));
        assert!(FILES[2].url.contains("rec_onnx") && FILES[2].url.ends_with("inference.onnx"));
        assert!(FILES[3].url.contains("rec_onnx") && FILES[3].url.ends_with("inference.yml"));
        assert_eq!(total_size(), 21_510_548);
        assert!(TABLE_FILES[0].url.contains("SLANet_plus_onnx") && TABLE_FILES[0].url.ends_with("inference.onnx"));
    }
}
