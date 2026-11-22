//! 配置文件的导出、导入、自动备份、恢复默认（需求第 10 节、M10.4）。
//!
//! - 导出按四类内容挑：常规设置、快捷键与工具栏、提示词、模型服务与备用链。API Key 不在 config.json 里
//!   （在系统凭据库），默认不导出；选择导出时用密码加密（PBKDF2-SHA256 派生密钥 + AES-256-GCM）。
//! - 导入时只取选中的几类，整块替换当前的对应设置（文件里没有的字段保留当前值，所以旧版本导出的文件也能导）。
//! - 备份是完整的 config.json 副本，文件名里带时间和原因，保留最近 20 份。

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::{parse, Config};

/// 导出文件的标识。
pub const FORMAT: &str = "snapmind-config";
/// 导出文件格式的版本。
pub const VERSION: u32 = 1;
/// PBKDF2 的迭代次数（OWASP 对 PBKDF2-HMAC-SHA256 的建议值）。
pub const ITERATIONS: u32 = 600_000;
/// 自动备份保留的份数。
pub const KEEP_BACKUPS: usize = 20;

#[derive(Debug, thiserror::Error)]
pub enum ConfigFileError {
    #[error("不是 SnapMind 导出的配置文件")]
    NotOurs,
    #[error("这个文件来自更新的 SnapMind（格式版本 {0}），请先升级")]
    TooNew(u32),
    #[error("密码不对，或文件已损坏")]
    BadPassword,
    #[error("读写文件失败：{0}")]
    Io(#[from] std::io::Error),
    #[error("文件内容不对：{0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Config(String),
}

/// 导出 / 导入的四类内容。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Section {
    /// 常规设置：外观、截图、保存、翻译偏好、录屏、历史保留方式、备份开关
    General,
    /// 快捷键与工具栏
    Keys,
    /// 提示词
    Prompts,
    /// 模型服务与备用链（含机器翻译的非敏感设置）
    Providers,
}

impl Section {
    pub const ALL: [Section; 4] = [Section::General, Section::Keys, Section::Prompts, Section::Providers];

    /// 这一类包含 Config 的哪些字段（camelCase，和 JSON 一致）。
    pub fn fields(self) -> &'static [&'static str] {
        match self {
            Section::General => &["appearance", "capture", "save", "translation", "record", "history", "backup", "ball", "update"],
            Section::Keys => &["hotkeys", "toolbar"],
            Section::Prompts => &["prompts"],
            Section::Providers => &["providers", "mt", "chains"],
        }
    }
}

/// 加密后的 API Key。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sealed {
    pub kdf: String,
    pub iterations: u32,
    pub salt: String,
    pub nonce: String,
    pub data: String,
}

/// 导出文件。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportFile {
    pub format: String,
    pub version: u32,
    pub app_version: String,
    /// 本地时间，如 2026-11-23 10:30:00
    pub exported_at: String,
    pub sections: Vec<Section>,
    /// 只含选中几类字段的配置
    pub config: Value,
    /// 加密的 API Key（账户名 → Key），没导出 Key 时为空
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Sealed>,
}

/// 生成导出文件。`secrets` 是要导出的 Key 和加密用的密码。
pub fn export(
    config: &Config,
    sections: &[Section],
    app_version: &str,
    now: NaiveDateTime,
    secrets: Option<(&BTreeMap<String, String>, &str)>,
) -> Result<ExportFile, ConfigFileError> {
    let full = serde_json::to_value(config)?;
    let mut out = Map::new();
    let mut picked: Vec<Section> = sections.to_vec();
    picked.sort();
    picked.dedup();
    for s in &picked {
        for f in s.fields() {
            if let Some(v) = full.get(*f) {
                out.insert((*f).to_string(), v.clone());
            }
        }
    }
    let sealed = match secrets {
        Some((map, pw)) if !map.is_empty() => Some(seal(map, pw, ITERATIONS)?),
        _ => None,
    };
    Ok(ExportFile {
        format: FORMAT.into(),
        version: VERSION,
        app_version: app_version.into(),
        exported_at: now.format("%Y-%m-%d %H:%M:%S").to_string(),
        sections: picked,
        config: Value::Object(out),
        secrets: sealed,
    })
}

/// 读导出文件（检查标识和版本）。
pub fn read_export(text: &str) -> Result<ExportFile, ConfigFileError> {
    let v: Value = serde_json::from_str(text.trim_start_matches('\u{feff}'))?;
    if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(ConfigFileError::NotOurs);
    }
    let file: ExportFile = serde_json::from_value(v)?;
    if file.version > VERSION {
        return Err(ConfigFileError::TooNew(file.version));
    }
    Ok(file)
}

/// 导入用的补丁：只含选中、且文件里确实有的那几类。每个字段整块给出，按 Merge Patch 应用时
/// 对象逐键覆盖、数组整体替换；文件里没有的字段保留当前值。
pub fn import_patch(file: &ExportFile, sections: &[Section]) -> Value {
    let mut out = Map::new();
    for s in sections {
        if !file.sections.contains(s) {
            continue;
        }
        for f in s.fields() {
            if let Some(v) = file.config.get(*f) {
                out.insert((*f).to_string(), v.clone());
            }
        }
    }
    Value::Object(out)
}

fn derive(password: &str, salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut key = [0u8; 32];
    pbkdf2::pbkdf2_hmac::<sha2::Sha256>(password.as_bytes(), salt, iterations, &mut key);
    key
}

/// 用密码加密一组 Key。
pub fn seal(map: &BTreeMap<String, String>, password: &str, iterations: u32) -> Result<Sealed, ConfigFileError> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 12];
    getrandom::getrandom(&mut salt).map_err(|e| ConfigFileError::Config(e.to_string()))?;
    getrandom::getrandom(&mut nonce).map_err(|e| ConfigFileError::Config(e.to_string()))?;
    let key = derive(password, &salt, iterations);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| ConfigFileError::Config(e.to_string()))?;
    let plain = serde_json::to_vec(map)?;
    let data = cipher
        .encrypt(Nonce::from_slice(&nonce), plain.as_ref())
        .map_err(|_| ConfigFileError::Config("加密失败".into()))?;
    Ok(Sealed {
        kdf: "pbkdf2-sha256".into(),
        iterations,
        salt: B64.encode(salt),
        nonce: B64.encode(nonce),
        data: B64.encode(data),
    })
}

/// 用密码解开 Key。密码不对时返回 BadPassword（AES-GCM 会校验完整性）。
pub fn open(sealed: &Sealed, password: &str) -> Result<BTreeMap<String, String>, ConfigFileError> {
    if sealed.kdf != "pbkdf2-sha256" {
        return Err(ConfigFileError::Config(format!("不认识的加密方式：{}", sealed.kdf)));
    }
    let bad = |_| ConfigFileError::BadPassword;
    let salt = B64.decode(&sealed.salt).map_err(bad)?;
    let nonce = B64.decode(&sealed.nonce).map_err(bad)?;
    let data = B64.decode(&sealed.data).map_err(bad)?;
    if nonce.len() != 12 {
        return Err(ConfigFileError::BadPassword);
    }
    let key = derive(password, &salt, sealed.iterations);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|e| ConfigFileError::Config(e.to_string()))?;
    let plain = cipher.decrypt(Nonce::from_slice(&nonce), data.as_ref()).map_err(|_| ConfigFileError::BadPassword)?;
    Ok(serde_json::from_slice(&plain)?)
}

/// 恢复默认设置：模型服务、机器翻译设置和备用链保留（需求第 10 节），历史的保留方式也保留
/// （历史记录本身在数据库里，不受影响；保留方式变回默认可能导致清理），其余还原。
pub fn reset_keeping(current: &Config) -> Config {
    Config {
        providers: current.providers.clone(),
        mt: current.mt.clone(),
        chains: current.chains.clone(),
        history: current.history.clone(),
        ..Config::default()
    }
}

// ---------------------------------------------------------------- 备份

/// 备份的原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupReason {
    /// 导入配置前
    Import,
    /// 恢复默认前
    Reset,
    /// 恢复某个备份前
    Restore,
    /// 修改重要设置前
    Change,
    /// 每周一次
    Weekly,
}

impl BackupReason {
    fn code(self) -> &'static str {
        match self {
            Self::Import => "import",
            Self::Reset => "reset",
            Self::Restore => "restore",
            Self::Change => "change",
            Self::Weekly => "weekly",
        }
    }
    fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "import" => Self::Import,
            "reset" => Self::Reset,
            "restore" => Self::Restore,
            "change" => Self::Change,
            "weekly" => Self::Weekly,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    /// 文件名（恢复时用它指定）
    pub name: String,
    /// 本地时间，如 2026-11-23 10:30:00
    pub time: String,
    pub reason: BackupReason,
    pub bytes: u64,
}

fn backup_name(now: NaiveDateTime, reason: BackupReason) -> String {
    format!("config-{}-{}.json", now.format("%Y%m%d-%H%M%S"), reason.code())
}

/// 从文件名解析出时间和原因。
fn parse_name(name: &str) -> Option<(NaiveDateTime, BackupReason)> {
    let rest = name.strip_prefix("config-")?.strip_suffix(".json")?;
    let (stamp, reason) = rest.rsplit_once('-')?;
    let t = NaiveDateTime::parse_from_str(stamp, "%Y%m%d-%H%M%S").ok()?;
    Some((t, BackupReason::parse(reason)?))
}

/// 写一份备份，并只保留最近的 KEEP_BACKUPS 份。
pub fn backup(dir: &Path, config: &Config, reason: BackupReason, now: NaiveDateTime) -> Result<BackupInfo, ConfigFileError> {
    fs::create_dir_all(dir)?;
    let mut name = backup_name(now, reason);
    // 同一秒内的第二份：时间往后挪一秒，免得覆盖
    let mut t = now;
    while dir.join(&name).exists() {
        t += chrono::Duration::seconds(1);
        name = backup_name(t, reason);
    }
    let text = serde_json::to_string_pretty(config)?;
    fs::write(dir.join(&name), &text)?;
    prune_backups(dir, KEEP_BACKUPS)?;
    Ok(BackupInfo { name, time: t.format("%Y-%m-%d %H:%M:%S").to_string(), reason, bytes: text.len() as u64 })
}

/// 备份列表，最新的在前。
pub fn list_backups(dir: &Path) -> Vec<BackupInfo> {
    let mut out: Vec<(NaiveDateTime, BackupInfo)> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let (t, reason) = parse_name(&name)?;
            let bytes = e.metadata().map(|m| m.len()).unwrap_or(0);
            Some((t, BackupInfo { name, time: t.format("%Y-%m-%d %H:%M:%S").to_string(), reason, bytes }))
        })
        .collect();
    out.sort_by(|a, b| b.0.cmp(&a.0));
    out.into_iter().map(|(_, b)| b).collect()
}

/// 最近一份备份（某种原因的，或任意原因）的时间。
pub fn latest_backup(dir: &Path, reason: Option<BackupReason>) -> Option<NaiveDateTime> {
    fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| parse_name(&e.file_name().to_string_lossy()))
        .filter(|(_, r)| reason.is_none_or(|x| x == *r))
        .map(|(t, _)| t)
        .max()
}

fn prune_backups(dir: &Path, keep: usize) -> Result<(), ConfigFileError> {
    for b in list_backups(dir).into_iter().skip(keep) {
        let _ = fs::remove_file(dir.join(&b.name));
    }
    Ok(())
}

/// 读一份备份。名字必须是备份目录里的备份文件（防止拿来读别的文件）。
pub fn load_backup(dir: &Path, name: &str) -> Result<Config, ConfigFileError> {
    if parse_name(name).is_none() || name.contains(['/', '\\']) {
        return Err(ConfigFileError::Config("不是备份文件".into()));
    }
    let text = fs::read_to_string(dir.join(name))?;
    parse(&text).map_err(|e| ConfigFileError::Config(e.to_string()))
}

/// 备份目录：配置目录下的 backups。
pub fn backup_dir(config_dir: &Path) -> PathBuf {
    config_dir.join("backups")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProviderConfig;

    fn t(s: &str) -> NaiveDateTime {
        NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").unwrap()
    }

    fn sample() -> Config {
        let mut c = Config::default();
        c.prompts.translate = "自定义翻译提示词".into();
        c.hotkeys.capture = "Ctrl+Alt+K".into();
        c.providers.push(ProviderConfig {
            id: "ds".into(),
            kind: crate::config::ProviderKind::Deepseek,
            name: "DeepSeek".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            models: vec![],
        });
        c
    }

    #[test]
    fn export_only_picked_sections_and_round_trip() {
        let c = sample();
        let f = export(&c, &[Section::Prompts, Section::Keys, Section::Prompts], "0.9.8", t("2026-11-23 10:00:00"), None).unwrap();
        assert_eq!(f.sections, vec![Section::Keys, Section::Prompts]);
        let obj = f.config.as_object().unwrap();
        assert!(obj.contains_key("prompts") && obj.contains_key("hotkeys") && obj.contains_key("toolbar"));
        assert!(!obj.contains_key("providers") && !obj.contains_key("appearance"));
        assert!(f.secrets.is_none());
        let back = read_export(&serde_json::to_string(&f).unwrap()).unwrap();
        assert_eq!(back, f);
    }

    #[test]
    fn import_patch_applies_only_chosen_and_present_sections() {
        let src = sample();
        let f = export(&src, &[Section::Prompts, Section::Providers], "0.9.8", t("2026-11-23 10:00:00"), None).unwrap();
        // 只选提示词和快捷键：快捷键文件里没有，不动
        let patch = import_patch(&f, &[Section::Prompts, Section::Keys]);
        let target = Config::default();
        let next = crate::config::apply_patch(&target, &patch).unwrap();
        assert_eq!(next.prompts.translate, "自定义翻译提示词");
        assert_eq!(next.hotkeys.capture, "Shift+F1");
        assert!(next.providers.is_empty());
    }

    #[test]
    fn rejects_foreign_and_newer_files() {
        assert!(matches!(read_export("{\"a\":1}"), Err(ConfigFileError::NotOurs)));
        let mut f = export(&Config::default(), &[Section::General], "x", t("2026-11-23 10:00:00"), None).unwrap();
        f.version = 99;
        assert!(matches!(read_export(&serde_json::to_string(&f).unwrap()), Err(ConfigFileError::TooNew(99))));
    }

    #[test]
    fn seal_and_open_with_password() {
        let mut m = BTreeMap::new();
        m.insert("ds".to_string(), "sk-secret-123".to_string());
        m.insert("deepl".to_string(), "abc:fx".to_string());
        let s = seal(&m, "正确的密码", 1_000).unwrap();
        assert!(!s.data.contains("sk-secret"));
        assert_eq!(open(&s, "正确的密码").unwrap(), m);
        assert!(matches!(open(&s, "错的"), Err(ConfigFileError::BadPassword)));
        // 每次加密的盐和随机数都不同
        assert_ne!(seal(&m, "正确的密码", 1_000).unwrap().salt, s.salt);
    }

    #[test]
    fn reset_keeps_providers_and_history_policy() {
        let mut c = sample();
        c.history.max_mb = 50;
        let r = reset_keeping(&c);
        assert_eq!(r.providers, c.providers);
        assert_eq!(r.history.max_mb, 50);
        assert_eq!(r.prompts.translate, "");
        assert_eq!(r.hotkeys.capture, "Shift+F1");
    }

    #[test]
    fn backups_are_listed_newest_first_and_pruned() {
        let dir = tempfile::tempdir().unwrap();
        let c = Config::default();
        let a = backup(dir.path(), &c, BackupReason::Weekly, t("2026-11-20 08:00:00")).unwrap();
        let b = backup(dir.path(), &c, BackupReason::Import, t("2026-11-23 10:00:00")).unwrap();
        let b2 = backup(dir.path(), &c, BackupReason::Import, t("2026-11-23 10:00:00")).unwrap();
        assert_ne!(b.name, b2.name, "同一秒的两份不覆盖");
        let list = list_backups(dir.path());
        assert_eq!(list.iter().map(|x| x.name.clone()).collect::<Vec<_>>(), vec![b2.name.clone(), b.name.clone(), a.name.clone()]);
        assert_eq!(latest_backup(dir.path(), Some(BackupReason::Weekly)), Some(t("2026-11-20 08:00:00")));
        assert_eq!(load_backup(dir.path(), &a.name).unwrap(), c);
        assert!(load_backup(dir.path(), "../config.json").is_err());
        for i in 0..25 {
            backup(dir.path(), &c, BackupReason::Change, t("2026-11-01 00:00:00") + chrono::Duration::minutes(i)).unwrap();
        }
        assert_eq!(list_backups(dir.path()).len(), KEEP_BACKUPS);
        // 最新的几份（11-23、11-20）留下，最早的 change 被删
        assert!(dir.path().join(&b2.name).exists());
    }
}
