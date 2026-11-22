//! 全局快捷键（需求第 2 节、M10.1）：按配置注册，配置一变就重新注册，并记下每一项的状态
//! （设置页显示：已生效 / 被其他程序占用 / 和另一项重复 / 写法不对）。
//!
//! 在设置页录制新组合键时先全部停用（否则按下 Shift+F1 会直接开始截图），录完再恢复。

use std::collections::HashMap;
use std::sync::Mutex;

use serde::Serialize;
use snapmind_core::config::HotkeysConfig;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::actions;
use crate::state::AppState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Action {
    Capture,
    PinClipboard,
    Selection,
    CaptureTranslate,
    CaptureRecognize,
}

impl Action {
    pub const ALL: [Action; 5] =
        [Action::Capture, Action::PinClipboard, Action::Selection, Action::CaptureTranslate, Action::CaptureRecognize];

    pub fn keys(self, cfg: &HotkeysConfig) -> &str {
        match self {
            Action::Capture => &cfg.capture,
            Action::PinClipboard => &cfg.pin_clipboard,
            Action::Selection => &cfg.selection,
            Action::CaptureTranslate => &cfg.capture_translate,
            Action::CaptureRecognize => &cfg.capture_recognize,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Capture => "截图",
            Action::PinClipboard => "贴图",
            Action::Selection => "划词翻译",
            Action::CaptureTranslate => "截图 OCR 后翻译",
            Action::CaptureRecognize => "直接截图 OCR",
        }
    }

    fn run<R: Runtime>(self, app: &AppHandle<R>) {
        match self {
            Action::Capture => actions::capture(app),
            Action::PinClipboard => actions::pin_clipboard(app),
            Action::Selection => actions::translate_selection(app),
            Action::CaptureTranslate => actions::capture_translate(app),
            Action::CaptureRecognize => actions::capture_recognize(app),
        }
    }
}

/// 一项快捷键的状态。
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub action: Action,
    pub keys: String,
    /// 已注册生效（没设置的也算 true）
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct HotkeyState {
    registered: Mutex<Vec<(Action, Shortcut)>>,
    status: Mutex<Vec<Status>>,
}

/// 检查一组快捷键：写法、彼此重复、和贴图穿透快捷键重复。返回每一项可以注册的 Shortcut 或错误。
pub fn check(cfg: &HotkeysConfig) -> Vec<(Action, String, Result<Option<Shortcut>, String>)> {
    let reserved: Option<Shortcut> = crate::pin::THROUGH_SHORTCUT.parse().ok();
    let mut seen: HashMap<u32, Action> = HashMap::new();
    Action::ALL
        .iter()
        .map(|&a| {
            let keys = a.keys(cfg).trim().to_string();
            if keys.is_empty() {
                return (a, keys, Ok(None));
            }
            let r = match keys.parse::<Shortcut>() {
                Err(_) => Err("无法识别的组合键".to_string()),
                Ok(s) if reserved.is_some_and(|r| r.id() == s.id()) => Err("和贴图的「鼠标穿透」快捷键重复".to_string()),
                Ok(s) => match seen.get(&s.id()) {
                    Some(other) => Err(format!("和「{}」重复", other.label())),
                    None => {
                        seen.insert(s.id(), a);
                        Ok(Some(s))
                    }
                },
            };
            (a, keys, r)
        })
        .collect()
}

fn unregister_all<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<HotkeyState>();
    let mut reg = state.registered.lock().expect("快捷键锁");
    for (_, s) in reg.drain(..) {
        let _ = app.global_shortcut().unregister(s);
    }
}

/// 按配置重新注册全部快捷键，返回每一项的状态。
pub fn apply<R: Runtime>(app: &AppHandle<R>, cfg: &HotkeysConfig) -> Vec<Status> {
    unregister_all(app);
    let state = app.state::<HotkeyState>();
    let mut reg = state.registered.lock().expect("快捷键锁");
    let mut out = Vec::new();
    for (action, keys, r) in check(cfg) {
        let st = match r {
            Ok(None) => Status { action, keys, ok: true, error: None },
            Ok(Some(s)) => match app.global_shortcut().register(s) {
                Ok(()) => {
                    reg.push((action, s));
                    Status { action, keys, ok: true, error: None }
                }
                Err(e) => {
                    eprintln!("注册快捷键 {keys} 失败：{e}");
                    Status { action, keys, ok: false, error: Some("已被其他程序占用".into()) }
                }
            },
            Err(e) => Status { action, keys, ok: false, error: Some(e) },
        };
        out.push(st);
    }
    drop(reg);
    *state.status.lock().expect("快捷键状态锁") = out.clone();
    crate::tray::sync_hotkeys(app, &cfg.capture, &cfg.pin_clipboard);
    out
}

/// 全局快捷键按下：是本模块注册的就执行对应动作并返回 true。
pub fn dispatch<R: Runtime>(app: &AppHandle<R>, shortcut: &Shortcut) -> bool {
    let action = {
        let state = app.state::<HotkeyState>();
        let reg = state.registered.lock().expect("快捷键锁");
        reg.iter().find(|(_, s)| s.id() == shortcut.id()).map(|(a, _)| *a)
    };
    match action {
        Some(a) => {
            a.run(app);
            true
        }
        None => false,
    }
}

#[tauri::command]
pub fn hotkeys_status(state: State<'_, HotkeyState>) -> Vec<Status> {
    state.status.lock().expect("快捷键状态锁").clone()
}

/// 设置页录制组合键时停用全部快捷键（on = true），录完恢复（返回最新状态）。
#[tauri::command]
pub fn hotkeys_suspend(app: AppHandle, config: State<'_, AppState>, on: bool) -> Vec<Status> {
    if on {
        unregister_all(&app);
        app.state::<HotkeyState>().status.lock().expect("快捷键状态锁").clone()
    } else {
        apply(&app, &config.config.get().hotkeys)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_bad_duplicate_and_reserved_keys() {
        let cfg = HotkeysConfig {
            capture: "Shift+F1".into(),
            pin_clipboard: "shift+f1".into(),
            selection: "Ctrl+Shift+X".into(),
            capture_translate: "Ctrl+Nope".into(),
            capture_recognize: String::new(),
        };
        let r = check(&cfg);
        assert!(matches!(r[0].2, Ok(Some(_))));
        assert_eq!(r[1].2.as_ref().unwrap_err(), "和「截图」重复");
        assert!(r[2].2.as_ref().unwrap_err().contains("鼠标穿透"));
        assert_eq!(r[3].2.as_ref().unwrap_err(), "无法识别的组合键");
        assert!(matches!(r[4].2, Ok(None)));
    }

    #[test]
    fn defaults_are_valid() {
        assert!(check(&HotkeysConfig::default()).iter().all(|(_, _, r)| matches!(r, Ok(Some(_)))));
    }
}
