//! 自动更新（M11.3）：tauri-plugin-updater，更新源是 GitHub Releases 上的 `latest.json`。
//!
//! - 检查：「关于」页手动点，或开着「自动检查更新」时启动 20 秒后和之后每 24 小时在后台查一次。
//! - 有新版本：托盘菜单顶上出现「更新到 x.y.z…」，「关于」页显示版本和说明；**用户点了才下载安装**。
//! - 安装：下载并校验签名（公钥在 tauri.conf.json，要求签名里带版本号，防止被降级），
//!   退出前把开着的贴图写进历史，然后交给安装程序（passive 模式，装完自动重新打开）。
//! - 公钥为空的构建（还没生成签名密钥）视为「没有配置更新源」，不检查。
//!
//! 状态统一放在 [`UpdateState`]，变化时广播 `update:state`，页面据此显示。

use snapmind_core::i18n::tr;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const EVENT: &str = "update:state";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    Checking,
    /// 已是最新
    Latest,
    /// 有新版本，等用户决定
    Available,
    Downloading,
    /// 下载完，正在交给安装程序
    Installing,
    Error,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// 这个构建有没有配置更新源（签名公钥）
    pub configured: bool,
    pub phase: Phase,
    pub current: String,
    /// 新版本号和说明
    pub version: Option<String>,
    pub notes: Option<String>,
    pub received: u64,
    pub total: u64,
    pub error: Option<String>,
    /// 上次检查完成的时间（Unix 毫秒）
    pub checked_at: Option<i64>,
}

#[derive(Default)]
pub struct UpdateState {
    status: Mutex<UpdateStatus>,
    /// 检查到的新版本（安装时用）
    pending: Mutex<Option<Update>>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// 公钥非空才算配置了更新源。
pub fn configured<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|v| v.as_str())
        .is_some_and(|s| !s.trim().is_empty())
}

pub fn status<R: Runtime>(app: &AppHandle<R>) -> UpdateStatus {
    let mut s = app.state::<UpdateState>().status.lock().expect("更新状态锁").clone();
    s.configured = configured(app);
    s.current = snapmind_core::version().to_string();
    s
}

fn set<R: Runtime>(app: &AppHandle<R>, f: impl FnOnce(&mut UpdateStatus)) {
    f(&mut app.state::<UpdateState>().status.lock().expect("更新状态锁"));
    let _ = app.emit(EVENT, status(app));
}

/// 检查一次。正在检查、下载或安装时不重复。
pub async fn check<R: Runtime>(app: &AppHandle<R>) -> UpdateStatus {
    if !configured(app) {
        return status(app);
    }
    {
        let st = app.state::<UpdateState>();
        let mut s = st.status.lock().expect("更新状态锁");
        if matches!(s.phase, Phase::Checking | Phase::Downloading | Phase::Installing) {
            return status(app);
        }
        s.phase = Phase::Checking;
        s.error = None;
    }
    let _ = app.emit(EVENT, status(app));
    let result = async {
        let mut builder = app.updater_builder();
        // 更新检查和下载同样遵守代理设置
        builder = match snapmind_providers::net::route().await {
            snapmind_providers::net::Route::System => builder,
            snapmind_providers::net::Route::Direct => builder.no_proxy(),
            snapmind_providers::net::Route::Proxy(url) => builder.proxy(url),
        };
        builder.build().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())
    }
    .await;
    match result {
        Ok(Some(update)) => {
            crate::log_info!("发现新版本 {}", update.version);
            let (v, notes) = (update.version.clone(), update.body.clone());
            *app.state::<UpdateState>().pending.lock().expect("更新状态锁") = Some(update);
            crate::tray::set_update(app, Some(&v));
            set(app, |s| {
                s.phase = Phase::Available;
                s.version = Some(v);
                s.notes = notes.filter(|n| !n.trim().is_empty());
                s.checked_at = Some(now_ms());
            });
        }
        Ok(None) => set(app, |s| {
            s.phase = Phase::Latest;
            s.checked_at = Some(now_ms());
        }),
        Err(e) => {
            crate::log_warn!("检查更新失败：{e}");
            set(app, |s| {
                s.phase = Phase::Error;
                s.error = Some(e);
            });
        }
    }
    status(app)
}

/// 下载并安装检查到的新版本。成功时安装程序接手、本进程退出，不会返回。
pub async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let update = app.state::<UpdateState>().pending.lock().expect("更新状态锁").clone().ok_or_else(|| tr("还没有检查到新版本"))?;
    set(app, |s| {
        s.phase = Phase::Downloading;
        s.received = 0;
        s.total = 0;
        s.error = None;
    });
    let progress = app.clone();
    // 先下载并校验签名（download 返回前已校验），通过了才动开着的贴图，再交给安装程序
    let r = async {
        let bytes = update
            .download(
                move |chunk, total| {
                    set(&progress, |s| {
                        s.received += chunk as u64;
                        s.total = total.unwrap_or(0);
                    });
                },
                || {},
            )
            .await?;
        crate::log_info!("新版本 {} 下载并校验完成，开始安装", update.version);
        set(app, |s| s.phase = Phase::Installing);
        // Windows 上安装程序启动后本进程会直接退出：先把开着的贴图写进历史
        crate::pin::close_all_for_exit(app);
        update.install(bytes)
    }
    .await;
    if let Err(e) = r {
        let e = e.to_string();
        crate::log_warn!("安装更新失败：{e}");
        set(app, |s| {
            s.phase = Phase::Error;
            s.error = Some(e.clone());
        });
        return Err(e);
    }
    // 非 Windows 平台装完不会自动退出：重启进入新版本
    app.restart();
}

/// 后台自动检查：启动 20 秒后一次，之后每 24 小时一次；每次都看当时的开关。
pub fn spawn_auto<R: Runtime>(app: &AppHandle<R>) {
    if !configured(app) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio_sleep(Duration::from_secs(20)).await;
        loop {
            let on = app.state::<crate::state::AppState>().config.get().update.auto_check;
            if on {
                check(&app).await;
            }
            tokio_sleep(Duration::from_secs(24 * 3600)).await;
        }
    });
}

async fn tokio_sleep(d: Duration) {
    // 不为一个 sleep 引入 tokio 依赖：在阻塞线程池里睡
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

#[tauri::command]
pub fn update_status(app: AppHandle) -> UpdateStatus {
    status(&app)
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> UpdateStatus {
    check(&app).await
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> Result<(), String> {
    install(&app).await
}
