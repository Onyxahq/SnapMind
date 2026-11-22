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
    /// 已添加的大模型服务商（需求 1.4）。API Key 不在这里，存在系统凭据库。
    pub providers: Vec<ProviderConfig>,
    /// 机器翻译服务的非敏感设置；Key 同样在系统凭据库（账户名见 [`MtKind::secret_account`]）。
    pub mt: MtConfig,
    /// 翻译、识别、一步到位三条备用链（需求 1.2）。
    pub chains: ChainsConfig,
    /// 四套提示词；空字符串表示用默认值（需求 1.3）。
    pub prompts: PromptsConfig,
    /// 翻译偏好（需求第 8 节；M6.3 继续补充）。
    pub translation: TranslationConfig,
    /// 录屏（需求 3.10）：录制前在工具条上选，选过的记住，下次沿用
    pub record: RecordConfig,
    /// 全局快捷键（需求第 2 节；设置里录制新组合键）
    pub hotkeys: HotkeysConfig,
    /// 截图和贴图工具条的按钮顺序、隐藏、收进「更多」（需求 3.6、第 9 节）
    pub toolbar: ToolbarConfig,
    /// 历史记录的保留方式（需求第 7 节）
    pub history: HistoryConfig,
    /// 配置的自动备份（需求第 10 节）
    pub backup: BackupConfig,
    /// 悬浮球（需求第 11 节：可选，默认关闭）
    pub ball: BallConfig,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CURRENT_VERSION,
            appearance: Appearance::default(),
            capture: CaptureConfig::default(),
            save: SaveConfig::default(),
            providers: Vec::new(),
            mt: MtConfig::default(),
            chains: ChainsConfig::default(),
            prompts: PromptsConfig::default(),
            translation: TranslationConfig::default(),
            record: RecordConfig::default(),
            hotkeys: HotkeysConfig::default(),
            toolbar: ToolbarConfig::default(),
            history: HistoryConfig::default(),
            backup: BackupConfig::default(),
            ball: BallConfig::default(),
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

/// 内置服务商种类（需求 1.4）。Ollama 和自定义服务都走 OpenAI 兼容接口。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Deepseek,
    Anthropic,
    Gemini,
    Openai,
    Openrouter,
    Ollama,
    Custom,
}

/// 模型标签：识别和一步到位只能选带「视觉」的模型。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelTag {
    Vision,
    Text,
    Reasoning,
    Local,
}

/// 服务商下的一个模型：只有勾选（enabled）的才出现在下拉框和备用链里。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelConfig {
    pub id: String,
    #[serde(default)]
    pub tags: Vec<ModelTag>,
    #[serde(default)]
    pub enabled: bool,
}

/// 一个已添加的服务商。`id` 在列表内唯一，也是系统凭据库里 Key 的账户名。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    pub id: String,
    pub kind: ProviderKind,
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub models: Vec<ModelConfig>,
}

/// 机器翻译服务（需求 1.1）。都用官方接口，需要各自的 Key。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MtKind {
    Deepl,
    Google,
    Microsoft,
}

impl MtKind {
    pub const ALL: [MtKind; 3] = [MtKind::Deepl, MtKind::Google, MtKind::Microsoft];

    /// 系统凭据库里的账户名（与大模型服务商的 id 分开，不会撞名）。
    pub fn secret_account(self) -> &'static str {
        match self {
            MtKind::Deepl => "mt-deepl",
            MtKind::Google => "mt-google",
            MtKind::Microsoft => "mt-microsoft",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MtConfig {
    /// 微软翻译资源所在区域（如 eastasia）；全局资源留空。
    pub microsoft_region: String,
}

/// 备用链里的一项。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BackendRef {
    /// 某个服务商下的模型（provider 是 [`ProviderConfig::id`]）
    Llm { provider: String, model: String },
    /// 机器翻译
    Mt { kind: MtKind },
    /// 本地 OCR（只在识别链里）
    LocalOcr,
    /// 一步到位链的最后一项：降级为「本地 OCR → 翻译链」
    Degrade,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ChainsConfig {
    /// 关掉时只用每条链的第 1 项
    pub fallback: bool,
    /// 每一项的超时（秒）：大模型是等到第一段文字，机器翻译是整个请求
    pub timeout_secs: u32,
    pub translate: Vec<BackendRef>,
    pub recognize: Vec<BackendRef>,
    pub oneshot: Vec<BackendRef>,
}

impl Default for ChainsConfig {
    fn default() -> Self {
        Self {
            fallback: true,
            timeout_secs: 8,
            translate: Vec::new(),
            recognize: vec![BackendRef::LocalOcr],
            oneshot: Vec::new(),
        }
    }
}

impl ChainsConfig {
    /// 实际使用的超时：限制在 3–60 秒，手改配置写了离谱的值也不至于卡死或秒失败。
    pub fn timeout(&self) -> std::time::Duration {
        std::time::Duration::from_secs(self.timeout_secs.clamp(3, 60) as u64)
    }
}

/// 用户改过的提示词；空字符串 = 用默认值。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PromptsConfig {
    pub recognize: String,
    pub translate: String,
    pub oneshot: String,
    pub markdown: String,
}

impl PromptsConfig {
    /// 实际使用的模板（没改过就是默认值）。
    pub fn template(&self, kind: crate::prompts::PromptKind) -> &str {
        use crate::prompts::PromptKind::*;
        let custom = match kind {
            Recognize => &self.recognize,
            Translate => &self.translate,
            Oneshot => &self.oneshot,
            Markdown => &self.markdown,
        };
        crate::prompts::effective(custom, kind)
    }
}

/// 原文已经是目标语言时怎么办。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SameLang {
    /// 翻译成第二语言
    Second,
    /// 不翻译，原样给出
    Skip,
}

/// 悬浮球：是否显示，以及上次贴靠的位置（窗口中心，虚拟桌面物理像素；空表示还没拖过，
/// 放在主屏右边缘约 2/3 高处）。每次显示时按这个点重新找显示器和左右边贴靠，屏幕变了也不会跑丢。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BallConfig {
    pub enabled: bool,
    pub pos: Option<[i32; 2]>,
}

/// 配置的自动备份：导入前、恢复前、恢复默认前总会备份；这个开关控制「修改重要设置前」和「每周」两种。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BackupConfig {
    pub auto: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self { auto: true }
    }
}

/// 历史记录的保留方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryMode {
    /// 不限制，手动清理
    Unlimited,
    /// 每类最多 max_count 条
    Count,
    /// 合计最多 max_mb MB，超出删最早的，贴图优先
    Size,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HistoryConfig {
    pub mode: HistoryMode,
    pub max_count: u32,
    pub max_mb: u32,
}

impl Default for HistoryConfig {
    fn default() -> Self {
        Self { mode: HistoryMode::Size, max_count: 500, max_mb: 1024 }
    }
}

/// 一条工具条的布局：按钮 id 的顺序（其中 `more` 是分隔线，它后面的按钮收进「⋯ 更多」）和隐藏的按钮。
/// 界面按这里排；不认识的 id 忽略，新版本新加的按钮自动出现在分隔线前面。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolbarLayout {
    pub order: Vec<String>,
    pub hidden: Vec<String>,
}

fn layout(ids: &[&str]) -> ToolbarLayout {
    ToolbarLayout { order: ids.iter().map(|s| s.to_string()).collect(), hidden: Vec::new() }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ToolbarConfig {
    pub capture: ToolbarLayout,
    pub pin: ToolbarLayout,
}

impl Default for ToolbarConfig {
    fn default() -> Self {
        Self {
            capture: layout(&[
                "rect", "pen", "arrow", "text", "number", "mosaic", "eraser", "undo", "long", "record", "ocr",
                "markdown", "translate", "pin", "save", "more", "formula", "table",
            ]),
            pin: layout(&["annotate", "save", "copy", "more", "through", "zoom100"]),
        }
    }
}

/// 全局快捷键。写法和 Tauri 一样（如 `Shift+F1`、`Ctrl+Alt+D`），空字符串表示不用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct HotkeysConfig {
    /// 截图
    pub capture: String,
    /// 贴图（剪贴板里的图片或文字）
    pub pin_clipboard: String,
    /// 划词翻译
    pub selection: String,
    /// 截图 → 识别 → 翻译
    pub capture_translate: String,
    /// 截图 → 识别（复制文字）
    pub capture_recognize: String,
}

impl Default for HotkeysConfig {
    fn default() -> Self {
        Self {
            capture: "Shift+F1".into(),
            pin_clipboard: "Shift+F3".into(),
            selection: "Ctrl+Alt+D".into(),
            capture_translate: "Ctrl+Shift+O".into(),
            capture_recognize: "Ctrl+Shift+S".into(),
        }
    }
}

/// 录屏格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordFormat {
    Mp4,
    Gif,
}

/// 录屏偏好。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct RecordConfig {
    pub format: RecordFormat,
    /// 帧率：15 / 30 / 60（GIF 最多 30）
    pub fps: u32,
    /// 录制系统声音（只对 MP4 有效）
    pub audio: bool,
}

impl Default for RecordConfig {
    fn default() -> Self {
        Self { format: RecordFormat::Mp4, fps: 30, audio: true }
    }
}

/// 翻译偏好（需求第 8 节）。语言用统一代码（见 [`crate::lang`]）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TranslationConfig {
    pub target_lang: String,
    /// 原文已是目标语言时翻成的语言
    pub second_lang: String,
    pub same_lang: SameLang,
    /// 复制 / 贴图时原文和译文按段落交替排列
    pub bilingual: bool,
    /// 保留换行、列表、代码块和 Markdown（只影响大模型提示词）
    pub keep_format: bool,
    /// 让大模型自动判断专业术语（只影响大模型提示词）
    pub auto_terms: bool,
    /// 不翻译列表：这些词保持原样（只影响大模型提示词）
    pub glossary: Vec<String>,
    /// 一步到位：识别和翻译交给同一个视觉模型，一次调用
    pub oneshot: bool,
    /// 截图翻译的结果用原位覆盖（译文画在原文位置上）而不是卡片；切换后记住
    pub overlay: bool,
}

impl Default for TranslationConfig {
    fn default() -> Self {
        Self {
            target_lang: "zh-Hans".into(),
            second_lang: "en".into(),
            same_lang: SameLang::Second,
            bilingual: true,
            keep_format: true,
            auto_terms: true,
            glossary: Vec::new(),
            oneshot: false,
            overlay: false,
        }
    }
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

    /// 整体换成另一份配置（恢复备份、恢复默认）。内容没变时不写盘，返回 `None`。
    pub fn replace(&self, next: Config) -> Result<Option<Config>, ConfigError> {
        let mut cur = self.current.write().expect("配置锁中毒");
        let mut next = next;
        next.version = cur.version;
        if next == *cur {
            return Ok(None);
        }
        self.write(&next)?;
        *cur = next.clone();
        Ok(Some(next))
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
                },
                "providers": [],
                "mt": { "microsoftRegion": "" },
                "chains": {
                    "fallback": true,
                    "timeoutSecs": 8,
                    "translate": [],
                    "recognize": [{ "type": "localOcr" }],
                    "oneshot": []
                },
                "prompts": { "recognize": "", "translate": "", "oneshot": "", "markdown": "" },
                "translation": {
                    "targetLang": "zh-Hans",
                    "secondLang": "en",
                    "sameLang": "second",
                    "bilingual": true,
                    "keepFormat": true,
                    "autoTerms": true,
                    "glossary": [],
                    "oneshot": false,
                    "overlay": false
                },
                "record": { "format": "mp4", "fps": 30, "audio": true },
                "hotkeys": {
                    "capture": "Shift+F1",
                    "pinClipboard": "Shift+F3",
                    "selection": "Ctrl+Alt+D",
                    "captureTranslate": "Ctrl+Shift+O",
                    "captureRecognize": "Ctrl+Shift+S"
                },
                "toolbar": {
                    "capture": {
                        "order": ["rect", "pen", "arrow", "text", "number", "mosaic", "eraser", "undo", "long", "record",
                                  "ocr", "markdown", "translate", "pin", "save", "more", "formula", "table"],
                        "hidden": []
                    },
                    "pin": { "order": ["annotate", "save", "copy", "more", "through", "zoom100"], "hidden": [] }
                },
                "history": { "mode": "size", "maxCount": 500, "maxMb": 1024 },
                "backup": { "auto": true },
                "ball": { "enabled": false, "pos": null }
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
    fn providers_roundtrip_and_patch_replaces_list() {
        let base = Config::default();
        let patch = json!({ "providers": [{
            "id": "deepseek", "kind": "deepseek", "name": "DeepSeek", "baseUrl": "https://api.deepseek.com/v1",
            "models": [{ "id": "deepseek-chat", "tags": ["text"], "enabled": true }]
        }]});
        let c = apply_patch(&base, &patch).unwrap();
        assert_eq!(c.providers[0].kind, ProviderKind::Deepseek);
        assert_eq!(c.providers[0].models[0].tags, vec![ModelTag::Text]);
        // 数组整体替换：传空数组就是全部移除
        let c2 = apply_patch(&c, &json!({ "providers": [] })).unwrap();
        assert!(c2.providers.is_empty());
        assert!(apply_patch(&base, &json!({ "providers": [{ "id": "x", "kind": "nope", "name": "", "baseUrl": "" }] })).is_err());
    }

    #[test]
    fn chain_refs_serialize_with_type_tag() {
        let c = apply_patch(&Config::default(), &json!({ "chains": { "translate": [
            { "type": "llm", "provider": "deepseek", "model": "deepseek-chat" },
            { "type": "mt", "kind": "deepl" }
        ], "oneshot": [{ "type": "degrade" }], "timeoutSecs": 999 }})).unwrap();
        assert_eq!(c.chains.translate[0], BackendRef::Llm { provider: "deepseek".into(), model: "deepseek-chat".into() });
        assert_eq!(c.chains.translate[1], BackendRef::Mt { kind: MtKind::Deepl });
        assert_eq!(c.chains.oneshot, vec![BackendRef::Degrade]);
        assert_eq!(c.chains.recognize, vec![BackendRef::LocalOcr]);
        assert_eq!(c.chains.timeout(), std::time::Duration::from_secs(60));
        assert!(c.chains.fallback);
    }

    #[test]
    fn prompts_default_until_customized_and_null_resets() {
        use crate::prompts::{default_prompt, PromptKind};
        let base = Config::default();
        assert_eq!(base.prompts.template(PromptKind::Translate), default_prompt(PromptKind::Translate));
        let c = apply_patch(&base, &json!({ "prompts": { "translate": "译成 {target_lang}：{text}" } })).unwrap();
        assert_eq!(c.prompts.template(PromptKind::Translate), "译成 {target_lang}：{text}");
        assert_eq!(c.prompts.template(PromptKind::Recognize), default_prompt(PromptKind::Recognize));
        // 恢复默认：传 null 删除该键
        let r = apply_patch(&c, &json!({ "prompts": { "translate": null } })).unwrap();
        assert_eq!(r.prompts.template(PromptKind::Translate), default_prompt(PromptKind::Translate));
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
