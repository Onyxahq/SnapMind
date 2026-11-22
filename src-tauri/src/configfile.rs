//! 配置文件（需求第 10 节、M10.4）：导出、导入、自动备份与恢复、恢复默认。格式和加密在
//! `snapmind_core::configfile`，这里负责对话框、系统凭据库里的 Key，以及修改配置后的连带动作。
//!
//! 所有对配置的修改都走 [`update`] / [`replace`]：重新注册快捷键、按新的保留方式清理历史、
//! 广播 config-changed，以及「修改重要设置前」的自动备份。

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Serialize;
use serde_json::Value;
use snapmind_core::config::{Config, MtKind};
use snapmind_core::configfile::{self as cf, BackupInfo, BackupReason, Section};
use snapmind_core::export::now_local;
use tauri::{AppHandle, Manager, Runtime, State, WebviewWindow};
use tauri_plugin_dialog::DialogExt;

use snapmind_platform::secrets;
use crate::state::{self, AppState};

/// 改了这些字段算「重要设置」，改之前先备份（最多每小时一份）。
const IMPORTANT: [&str; 6] = ["providers", "mt", "chains", "hotkeys", "prompts", "toolbar"];

fn backup_dir<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    Ok(cf::backup_dir(&state::config_dir(app).map_err(|e| e.to_string())?))
}

fn backup_now<R: Runtime>(app: &AppHandle<R>, config: &Config, reason: BackupReason) -> Result<BackupInfo, String> {
    cf::backup(&backup_dir(app)?, config, reason, now_local()).map_err(|e| e.to_string())
}

const HOUR: i64 = 3600;
const WEEK: i64 = 7 * 24 * HOUR;

/// 距上一份某种备份已经过了多少秒（没有时为 None）。
fn since_last<R: Runtime>(app: &AppHandle<R>, reason: Option<BackupReason>) -> Option<i64> {
    let dir = backup_dir(app).ok()?;
    cf::latest_backup(&dir, reason).map(|t| (now_local() - t).num_seconds())
}

/// 每周一次的自动备份（启动时和每次修改配置时检查）。
pub fn weekly_backup<R: Runtime>(app: &AppHandle<R>) {
    let config = app.state::<AppState>().config.get();
    if !config.backup.auto {
        return;
    }
    if since_last(app, None).is_none_or(|s| s >= WEEK) {
        if let Err(e) = backup_now(app, &config, BackupReason::Weekly) {
            eprintln!("每周备份配置失败：{e}");
        }
    }
}

/// 配置改了之后的连带动作。
fn after_change<R: Runtime>(app: &AppHandle<R>, prev: &Config, next: &Config) {
    if next.hotkeys != prev.hotkeys {
        crate::hotkeys::apply(app, &next.hotkeys);
    }
    if next.history != prev.history {
        let app = app.clone();
        std::thread::spawn(move || crate::history::prune(&app));
    }
    state::on_config_changed(app, next);
}

/// 用 JSON Merge Patch 修改配置（设置页的每一次修改都走这里）。
pub fn update<R: Runtime>(app: &AppHandle<R>, patch: &Value) -> Result<Config, String> {
    apply(app, patch, true)
}

/// `auto_backup`：是否做「修改重要设置前」的自动备份（导入时已经单独备份过，不再重复）。
fn apply<R: Runtime>(app: &AppHandle<R>, patch: &Value, auto_backup: bool) -> Result<Config, String> {
    let state = app.state::<AppState>();
    let prev = state.config.get();
    let touches_important = patch.as_object().is_some_and(|o| o.keys().any(|k| IMPORTANT.contains(&k.as_str())));
    if auto_backup && prev.backup.auto && touches_important && since_last(app, Some(BackupReason::Change)).is_none_or(|s| s >= HOUR) {
        // 只在真的会变时备份
        if snapmind_core::config::apply_patch(&prev, patch).is_ok_and(|n| n != prev) {
            if let Err(e) = backup_now(app, &prev, BackupReason::Change) {
                eprintln!("修改前备份配置失败：{e}");
            }
        }
    }
    match state.config.update(patch).map_err(|e| e.to_string())? {
        Some(next) => {
            after_change(app, &prev, &next);
            weekly_backup(app);
            Ok(next)
        }
        None => Ok(prev),
    }
}

/// 整体换成另一份配置（恢复备份、恢复默认）。
fn replace<R: Runtime>(app: &AppHandle<R>, next: Config) -> Result<Config, String> {
    let state = app.state::<AppState>();
    let prev = state.config.get();
    match state.config.replace(next).map_err(|e| e.to_string())? {
        Some(next) => {
            after_change(app, &prev, &next);
            Ok(next)
        }
        None => Ok(prev),
    }
}

/// 系统凭据库里所有已填的 Key（账户名 → Key）：各服务商 + 机器翻译。
fn collect_keys(config: &Config) -> BTreeMap<String, String> {
    let accounts = config.providers.iter().map(|p| p.id.clone()).chain(MtKind::ALL.iter().map(|k| k.secret_account().to_string()));
    accounts.filter_map(|a| secrets::get(&a).ok().flatten().filter(|k| !k.is_empty()).map(|k| (a, k))).collect()
}

/// 导出配置：系统另存为，返回保存的路径（取消时为 None）。`password` 有值时连 API Key 一起加密导出
/// （只在选了「模型服务与备用链」时才有意义）。
#[tauri::command]
pub async fn config_export(
    app: AppHandle,
    window: WebviewWindow,
    sections: Vec<Section>,
    password: Option<String>,
) -> Result<Option<String>, String> {
    if sections.is_empty() {
        return Err("至少选一项内容".into());
    }
    let config = app.state::<AppState>().config.get();
    let keys = match password.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(_) if sections.contains(&Section::Providers) => Some(collect_keys(&config)),
        _ => None,
    };
    let version = snapmind_core::version().to_string();
    let pw = password.unwrap_or_default();
    let file = tauri::async_runtime::spawn_blocking(move || {
        cf::export(&config, &sections, &version, now_local(), keys.as_ref().map(|k| (k, pw.as_str()))).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;
    let name = format!("SnapMind-配置-{}.json", now_local().format("%Y%m%d"));
    let picked = tauri::async_runtime::spawn_blocking(move || {
        window
            .dialog()
            .file()
            .set_parent(&window)
            .set_file_name(name)
            .add_filter("SnapMind 配置", &["json"])
            .blocking_save_file()
            .and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| e.to_string())?;
    let Some(path) = picked else { return Ok(None) };
    let text = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
    std::fs::write(&path, text + "\n").map_err(|e| format!("保存失败：{e}"))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

/// 导入前的预览：文件里有哪几类内容、有没有加密的 Key。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    pub path: String,
    pub sections: Vec<Section>,
    pub has_keys: bool,
    pub app_version: String,
    pub exported_at: String,
}

fn read_file(path: &str) -> Result<cf::ExportFile, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("读不了这个文件：{e}"))?;
    cf::read_export(&text).map_err(|e| e.to_string())
}

/// 读一个导出文件（拖进窗口时）。
#[tauri::command]
pub fn config_import_read(path: String) -> Result<ImportPreview, String> {
    let f = read_file(&path)?;
    Ok(ImportPreview { path, sections: f.sections, has_keys: f.secrets.is_some(), app_version: f.app_version, exported_at: f.exported_at })
}

/// 选一个导出文件（点击选择时），取消时为 None。
#[tauri::command]
pub async fn config_import_pick(window: WebviewWindow) -> Result<Option<ImportPreview>, String> {
    let picked = tauri::async_runtime::spawn_blocking(move || {
        window
            .dialog()
            .file()
            .set_parent(&window)
            .add_filter("SnapMind 配置", &["json"])
            .blocking_pick_file()
            .and_then(|p| p.into_path().ok())
    })
    .await
    .map_err(|e| e.to_string())?;
    match picked {
        Some(p) => config_import_read(p.to_string_lossy().into_owned()).map(Some),
        None => Ok(None),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    /// 导入前的备份
    pub backup: String,
    /// 写回凭据库的 Key 个数
    pub keys: usize,
}

/// 导入：先备份当前配置，再把选中的几类整块替换；有 Key 且给了密码时写回系统凭据库。
/// 密码不对时什么都不改。
#[tauri::command]
pub async fn config_import(app: AppHandle, path: String, sections: Vec<Section>, password: Option<String>) -> Result<ImportResult, String> {
    if sections.is_empty() {
        return Err("至少选一项内容".into());
    }
    let file = read_file(&path)?;
    let keys = match (&file.secrets, password.as_deref().map(str::trim).filter(|p| !p.is_empty())) {
        (Some(sealed), Some(pw)) if sections.contains(&Section::Providers) => {
            let (sealed, pw) = (sealed.clone(), pw.to_string());
            Some(
                tauri::async_runtime::spawn_blocking(move || cf::open(&sealed, &pw))
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| e.to_string())?,
            )
        }
        _ => None,
    };
    let patch = cf::import_patch(&file, &sections);
    let current = app.state::<AppState>().config.get();
    // 先校验补丁能用，再备份、再改
    snapmind_core::config::apply_patch(&current, &patch).map_err(|e| format!("文件里的设置不合法：{e}"))?;
    let backup = backup_now(&app, &current, BackupReason::Import)?;
    apply(&app, &patch, false)?;
    let mut n = 0;
    for (account, key) in keys.unwrap_or_default() {
        match secrets::set(&account, &key) {
            Ok(()) => n += 1,
            Err(e) => eprintln!("写入 Key（{account}）失败：{e}"),
        }
    }
    Ok(ImportResult { backup: backup.name, keys: n })
}

/// 备份列表，最新的在前。
#[tauri::command]
pub fn config_backups(app: AppHandle) -> Result<Vec<BackupInfo>, String> {
    Ok(cf::list_backups(&backup_dir(&app)?))
}

/// 恢复某个备份（先备份当前配置）。
#[tauri::command]
pub fn config_restore(app: AppHandle, state: State<'_, AppState>, name: String) -> Result<(), String> {
    let dir = backup_dir(&app)?;
    let cfg = cf::load_backup(&dir, &name).map_err(|e| e.to_string())?;
    backup_now(&app, &state.config.get(), BackupReason::Restore)?;
    replace(&app, cfg).map(|_| ())
}

/// 恢复默认设置：模型服务、备用链和历史保留方式保留，其余还原（先备份）。
#[tauri::command]
pub fn config_reset(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let cur = state.config.get();
    backup_now(&app, &cur, BackupReason::Reset)?;
    replace(&app, cf::reset_keeping(&cur)).map(|_| ())
}
