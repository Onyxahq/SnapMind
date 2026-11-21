//! 配置：`config.json` 的结构、默认值、版本迁移、读写。
//!
//! 修改配置只有一种方式：提交一个 JSON Merge Patch（RFC 7386），
//! 合并到当前配置后整体校验，通过才落盘。这样前端只需要一个「更新」命令，
//! 各个设置页互不干扰，非法值也进不了文件。

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::export::{ImageFormat, DEFAULT_TEMPLATE};

/// 当前配置版本。结构有不兼容变化时加一，并在 [`migrate`] 里补一步迁移。
pub const CURRENT_VERSION: u32 = 1;

/// 配置文件名。
pub const FILE_NAME: &str = "config.json";

// ---------------------------------------------------------------- 结构

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub appearance: Appearance,
    pub capture: CaptureConfig,
    pub save: SaveConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            appearance: Appearance::default(),
            capture: CaptureConfig::default(),
            save: SaveConfig::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Appearance {
    pub theme: ThemeMode,
    pub accent: Accent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    #[default]
    Blue,
    Sky,
    Violet,
    Green,
}

/// 截图编辑态的设置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CaptureConfig {
    /// 放大镜里颜色值的格式，按 P 切换后记住。
    pub color_format: ColorFormat,
    /// 是否显示操作提示。截图时按 H 切换，会记住；设置里也有同一个开关。
    pub show_hints: bool,
    /// 有未保存的标注时退出前是否确认（确认框里可勾选「不再询问」）。
    pub confirm_discard: bool,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self { color_format: ColorFormat::default(), show_hints: true, confirm_discard: true }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ColorFormat {
    #[default]
    Hex,
    Rgb,
    Hsl,
}

/// 保存截图的设置（需求 3.7）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SaveConfig {
    /// 默认保存目录；空字符串表示「图片\SnapMind」。
    pub dir: String,
    pub format: ImageFormat,
    /// JPG / WebP 的质量，1–100。
    pub quality: u8,
    /// 文件名模板，花括号里写时间格式，如 `SnapMind_{yyyyMMdd_HHmmss}`。
    pub filename_template: String,
    /// Ctrl+S 的行为。
    pub ctrl_s: CtrlSAction,
    /// 用户在保存窗口里「＋添加」的常用位置。
    pub extra_dirs: Vec<String>,
}

impl Default for SaveConfig {
    fn default() -> Self {
        Self {
            dir: String::new(),
            format: ImageFormat::Png,
            quality: 90,
            filename_template: DEFAULT_TEMPLATE.to_string(),
            ctrl_s: CtrlSAction::Window,
            extra_dirs: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CtrlSAction {
    /// SnapMind 保存窗口
    #[default]
    Window,
    /// 系统另存为对话框
    System,
    /// 直接保存到默认目录
    Direct,
}

// ---------------------------------------------------------------- 错误

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("读写配置文件失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("配置内容无效：{0}")]
    Invalid(#[from] serde_json::Error),
    #[error("配置更新必须是 JSON 对象")]
    PatchNotObject,
}

// ---------------------------------------------------------------- 纯函数

/// JSON Merge Patch（RFC 7386）：对象逐键合并，`null` 表示删除，其他值整体替换。
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(patch_map) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = Value::Object(Map::new());
    }
    let target_map = target.as_object_mut().expect("刚确保是对象");
    for (k, v) in patch_map {
        if v.is_null() {
            target_map.remove(k);
        } else {
            merge_patch(target_map.entry(k.clone()).or_insert(Value::Null), v);
        }
    }
}

/// 把任意版本的配置 JSON 迁移到 [`CURRENT_VERSION`]。
///
/// - 没有 `version` 字段：视为第 1 版（第一个发布版本）。
/// - 版本号比当前程序新：原样返回，按当前结构宽松读取（未知字段忽略、缺的字段取默认值）。
pub fn migrate(mut v: Value) -> Value {
    let mut ver = v.get("version").and_then(Value::as_u64).unwrap_or(1) as u32;
    while ver < CURRENT_VERSION {
        // 以后在这里按版本逐步迁移，例如：
        // 1 => { rename(&mut v, "/a", "/b"); }
        ver += 1;
    }
    if ver <= CURRENT_VERSION {
        if let Value::Object(m) = &mut v {
            m.insert("version".into(), Value::from(CURRENT_VERSION));
        }
    }
    v
}

/// 从 JSON 文本解析配置（先迁移，再按结构读取）。
/// 允许开头带 UTF-8 BOM：用记事本、PowerShell 改过的配置常常带着它。
pub fn parse(text: &str) -> Result<Config, ConfigError> {
    let raw: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    Ok(serde_json::from_value(migrate(raw))?)
}

/// 在 `base` 上应用补丁并校验，返回新配置。`version` 字段不允许通过补丁修改。
pub fn apply_patch(base: &Config, patch: &Value) -> Result<Config, ConfigError> {
    if !patch.is_object() {
        return Err(ConfigError::PatchNotObject);
    }
    let mut v = serde_json::to_value(base)?;
    merge_patch(&mut v, patch);
    let mut next: Config = serde_json::from_value(v)?;
    next.version = base.version;
    Ok(next)
}

// ---------------------------------------------------------------- 存储

/// 加载时发生的情况，供外壳记日志或提示用户。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    /// 正常读取。
    Loaded,
    /// 文件不存在，已用默认值创建。
    Created,
    /// 文件损坏，已改名为备份并用默认值重建。
    Recovered { backup: PathBuf },
}

/// 配置的唯一持有者：内存中一份，修改时同步写盘。
pub struct ConfigStore {
    path: PathBuf,
    current: RwLock<Config>,
}

impl ConfigStore {
    /// 打开 `dir/config.json`。不存在就创建；损坏就备份后重建，不让程序因为配置起不来。
    pub fn open(dir: &Path) -> Result<(Self, LoadOutcome), ConfigError> {
        fs::create_dir_all(dir)?;
        let path = dir.join(FILE_NAME);
        let (config, outcome) = match fs::read_to_string(&path) {
            Ok(text) => match parse(&text) {
                Ok(c) => (c, LoadOutcome::Loaded),
                Err(_) => {
                    let backup = broken_backup_path(&path);
                    fs::rename(&path, &backup)?;
                    (Config::default(), LoadOutcome::Recovered { backup })
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                (Config::default(), LoadOutcome::Created)
            }
            Err(e) => return Err(e.into()),
        };
        let store = Self { path, current: RwLock::new(config) };
        if outcome != LoadOutcome::Loaded {
            store.write(&store.get())?;
        }
        Ok((store, outcome))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self) -> Config {
        self.current.read().expect("配置锁中毒").clone()
    }

    /// 应用补丁，校验通过后写盘并返回新配置；内容没变时不写盘，返回 `None`。
    pub fn update(&self, patch: &Value) -> Result<Option<Config>, ConfigError> {
        let mut cur = self.current.write().expect("配置锁中毒");
        let next = apply_patch(&cur, patch)?;
        if next == *cur {
            return Ok(None);
        }
        self.write(&next)?;
        *cur = next.clone();
        Ok(Some(next))
    }

    /// 先写临时文件再改名，断电或崩溃时不会留下写了一半的配置。
    fn write(&self, config: &Config) -> Result<(), ConfigError> {
        let tmp = self.path.with_extension("json.tmp");
        let text = serde_json::to_string_pretty(config)?;
        fs::write(&tmp, text + "\n")?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

fn broken_backup_path(path: &Path) -> PathBuf {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    path.with_file_name(format!("config.broken-{secs}.json"))
}

// ---------------------------------------------------------------- 测试

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_follow_the_spec() {
        let c = Config::default();
        assert_eq!(c.version, CURRENT_VERSION);
        assert_eq!(c.appearance.theme, ThemeMode::System);
        assert_eq!(c.appearance.accent, Accent::Blue);
    }

    #[test]
    fn serializes_as_camel_case_lowercase_enums() {
        let v = serde_json::to_value(Config::default()).unwrap();
        assert_eq!(
            v,
            json!({
                "version": 1,
                "appearance": { "theme": "system", "accent": "blue" },
                "capture": { "colorFormat": "hex", "showHints": true, "confirmDiscard": true },
                "save": {
                    "dir": "",
                    "format": "png",
                    "quality": 90,
                    "filenameTemplate": "SnapMind_{yyyyMMdd_HHmmss}",
                    "ctrlS": "window",
                    "extraDirs": []
                }
            })
        );
    }

    #[test]
    fn merge_patch_follows_rfc7386() {
        let mut t = json!({ "a": { "b": 1, "c": 2 }, "d": [1, 2] });
        merge_patch(&mut t, &json!({ "a": { "b": 9, "c": null }, "d": [3], "e": "x" }));
        assert_eq!(t, json!({ "a": { "b": 9 }, "d": [3], "e": "x" }));
    }

    #[test]
    fn hints_default_on_even_when_capture_section_is_partial() {
        let c = parse(r#"{ "capture": { "colorFormat": "rgb" } }"#).unwrap();
        assert!(c.capture.show_hints);
        assert_eq!(c.capture.color_format, ColorFormat::Rgb);
    }

    #[test]
    fn patch_changes_only_given_fields() {
        let c = apply_patch(&Config::default(), &json!({ "appearance": { "theme": "dark" } })).unwrap();
        assert_eq!(c.appearance.theme, ThemeMode::Dark);
        assert_eq!(c.appearance.accent, Accent::Blue);
    }

    #[test]
    fn invalid_patch_is_rejected() {
        let base = Config::default();
        assert!(apply_patch(&base, &json!({ "appearance": { "accent": "pink" } })).is_err());
        assert!(apply_patch(&base, &json!("dark")).is_err());
    }

    #[test]
    fn patch_cannot_change_version() {
        let c = apply_patch(&Config::default(), &json!({ "version": 99 })).unwrap();
        assert_eq!(c.version, CURRENT_VERSION);
    }

    #[test]
    fn missing_fields_take_defaults_and_unknown_fields_are_ignored() {
        let c = parse(r#"{ "appearance": { "accent": "green" }, "fromTheFuture": true }"#).unwrap();
        assert_eq!(c.appearance.accent, Accent::Green);
        assert_eq!(c.appearance.theme, ThemeMode::System);
        assert_eq!(c.version, CURRENT_VERSION);
    }

    #[test]
    fn utf8_bom_is_accepted() {
        let c = parse("\u{feff}{ \"appearance\": { \"theme\": \"dark\" } }").unwrap();
        assert_eq!(c.appearance.theme, ThemeMode::Dark);
    }

    #[test]
    fn newer_version_is_read_leniently_without_downgrading() {
        let v = migrate(json!({ "version": CURRENT_VERSION + 5 }));
        assert_eq!(v["version"], json!(CURRENT_VERSION + 5));
    }

    #[test]
    fn store_creates_file_on_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let (store, outcome) = ConfigStore::open(dir.path()).unwrap();
        assert_eq!(outcome, LoadOutcome::Created);
        assert_eq!(parse(&fs::read_to_string(store.path()).unwrap()).unwrap(), Config::default());
    }

    #[test]
    fn store_persists_updates_and_skips_no_ops() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = ConfigStore::open(dir.path()).unwrap();
        let patch = json!({ "appearance": { "accent": "violet" } });
        assert!(store.update(&patch).unwrap().is_some());
        assert!(store.update(&patch).unwrap().is_none(), "相同内容不应再次写盘");

        let (reopened, outcome) = ConfigStore::open(dir.path()).unwrap();
        assert_eq!(outcome, LoadOutcome::Loaded);
        assert_eq!(reopened.get().appearance.accent, Accent::Violet);
        assert!(!dir.path().join("config.json.tmp").exists());
    }

    #[test]
    fn rejected_update_leaves_file_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let (store, _) = ConfigStore::open(dir.path()).unwrap();
        let before = fs::read_to_string(store.path()).unwrap();
        assert!(store.update(&json!({ "appearance": { "theme": 3 } })).is_err());
        assert_eq!(fs::read_to_string(store.path()).unwrap(), before);
        assert_eq!(store.get(), Config::default());
    }

    #[test]
    fn corrupt_file_is_backed_up_and_rebuilt() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(FILE_NAME), "{ not json").unwrap();
        let (store, outcome) = ConfigStore::open(dir.path()).unwrap();
        let LoadOutcome::Recovered { backup } = outcome else { panic!("应当触发恢复") };
        assert_eq!(fs::read_to_string(backup).unwrap(), "{ not json");
        assert_eq!(store.get(), Config::default());
    }
}
