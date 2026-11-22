//! 截图翻译（需求 3.8、M6.1）：识别链 → 翻译链，结果流式推给发起的窗口。
//!
//! 每个任务有前端给的 id；同一个 id 再次发起或调用 `task_cancel` 都会中止旧任务
//! （中止就是丢弃 future，连接随之关闭）。事件 `task:event` 只发给发起任务的窗口。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use snapmind_core::config::{BackendRef, Config};
use snapmind_core::export::{self, ImageFormat, RgbaImage};
use snapmind_core::prompts::{render, PromptKind, PromptVars};
use snapmind_platform::secrets;
use snapmind_providers::{
    describe_failures, resolve, run_text_chain, Backend, ChainEvent, ChainRun, Message, Part, ProviderError, TextJob,
};
use tauri::{AppHandle, Emitter, Manager, Runtime, State, WebviewWindow};

use crate::export::Region;
use crate::state::AppState;

const EVENT: &str = "task:event";
/// 送给视觉模型的图片最长边：再大只是多花 token，识别率不会更好。
const MAX_IMAGE_SIDE: u32 = 2048;

#[derive(Default)]
pub struct TaskState {
    tasks: Mutex<HashMap<String, tauri::async_runtime::JoinHandle<()>>>,
}

impl TaskState {
    fn replace(&self, id: &str, handle: tauri::async_runtime::JoinHandle<()>) {
        if let Some(old) = self.tasks.lock().expect("任务锁").insert(id.to_string(), handle) {
            old.abort();
        }
    }

    fn cancel(&self, id: &str) {
        if let Some(old) = self.tasks.lock().expect("任务锁").remove(id) {
            old.abort();
        }
    }
}

/// 推给前端的事件。`kind`：
/// - `stage`：开始某一步（`stage` = recognize / translate，`text` = 正在用的后端）
/// - `src` / `dst`：原文 / 译文的增量；`src-reset` / `dst-reset`：切换后端，清掉半截输出
/// - `done`：完成，`text` = 说明（实际使用的后端、切换原因、耗时）
/// - `error`：失败，`text` = 原因
#[derive(Debug, Clone, Serialize)]
pub struct TaskEvent {
    id: String,
    kind: &'static str,
    stage: &'static str,
    text: String,
}

struct Emit<R: Runtime> {
    win: WebviewWindow<R>,
    id: String,
}

impl<R: Runtime> Emit<R> {
    fn send(&self, kind: &'static str, stage: &'static str, text: impl Into<String>) {
        let ev = TaskEvent { id: self.id.clone(), kind, stage, text: text.into() };
        let _ = self.win.emit_to(self.win.label(), EVENT, ev);
    }
}

/// 等比缩小到最长边不超过 `max`。
pub fn fit_size(w: u32, h: u32, max: u32) -> (u32, u32) {
    let long = w.max(h);
    if long <= max {
        return (w, h);
    }
    let k = max as f64 / long as f64;
    (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1))
}

fn key_of(account: &str) -> Option<String> {
    secrets::get(account).ok().flatten()
}

fn secs(t: Instant) -> String {
    format!("{:.1} s", t.elapsed().as_secs_f64())
}

/// 识别链：本地 OCR（M4 前不可用）或视觉模型。返回（原文，说明）。
async fn recognize<R: Runtime>(cfg: &Config, png: Vec<u8>, out: &Emit<R>) -> Result<(String, String), String> {
    let chains = &cfg.chains;
    // 本地 OCR 不是大模型后端，单独占位；其余按设置解析
    enum Step {
        LocalOcr,
        Llm(Backend),
    }
    let steps: Vec<Step> = chains
        .recognize
        .iter()
        .filter_map(|r| match r {
            BackendRef::LocalOcr => Some(Step::LocalOcr),
            other => resolve(std::slice::from_ref(other), &cfg.providers, &cfg.mt, key_of).pop().filter(Backend::is_llm).map(Step::Llm),
        })
        .collect();
    if steps.is_empty() {
        return Err("还没有识别后端：在设置的「备用链 · 识别」里添加一个视觉模型".into());
    }
    let labels: Vec<String> = steps
        .iter()
        .map(|s| match s {
            Step::LocalOcr => "本地 OCR".to_string(),
            Step::Llm(b) => b.label(),
        })
        .collect();
    let prompt = render(
        cfg.prompts.template(PromptKind::Recognize),
        PromptKind::Recognize,
        &PromptVars { target_lang: &cfg.translation.target_lang, ..Default::default() },
    );
    let job = TextJob {
        messages: vec![Message::user(vec![Part::Image { mime: "image/png".into(), data: png }, Part::Text(prompt)])],
        temperature: Some(0.0),
        ..Default::default()
    };
    let timeout = chains.timeout();
    let mut on_delta = |d: &str| out.send("src", "recognize", d);
    let mut run = ChainRun::new(labels, chains.fallback);
    for i in 0..run.len() {
        out.send("stage", "recognize", run.label(i));
        let r = match &steps[i] {
            // M4 装好本地 OCR 之前，识别链里的本地 OCR 直接报不可用并切到下一个
            Step::LocalOcr => Err(ProviderError::Unavailable("本地 OCR 还没有安装".into())),
            Step::Llm(b) => b.run_text(&job, timeout, &mut on_delta).await,
        };
        match r {
            Ok(text) => {
                let s = run.succeed(i, text);
                return Ok((s.value.trim().to_string(), s.describe()));
            }
            Err(e) => {
                run.fail(i, e);
                out.send("src-reset", "recognize", "");
            }
        }
    }
    Err(format!("识别失败：{}", describe_failures(&run.failures())))
}

/// 翻译链。返回（译文，说明）。
async fn translate<R: Runtime>(cfg: &Config, source: &str, out: &Emit<R>) -> Result<(String, String), String> {
    let chains = &cfg.chains;
    let backends = resolve(&chains.translate, &cfg.providers, &cfg.mt, key_of);
    if backends.is_empty() {
        return Err("还没有翻译后端：在设置的「备用链 · 翻译」里添加大模型或机器翻译".into());
    }
    let target = cfg.translation.target_lang.as_str();
    let prompt = render(
        cfg.prompts.template(PromptKind::Translate),
        PromptKind::Translate,
        &PromptVars { target_lang: target, source_lang: None, glossary: &[], text: source },
    );
    let job = TextJob {
        messages: vec![Message::user_text(prompt)],
        temperature: Some(0.2),
        text: source.to_string(),
        source: None,
        target: target.to_string(),
        ..Default::default()
    };
    let r = run_text_chain(
        &backends,
        chains.fallback,
        &job,
        chains.timeout(),
        |d| out.send("dst", "translate", d),
        |e| match e {
            ChainEvent::Start { label, .. } => out.send("stage", "translate", label),
            ChainEvent::Failed(_) => out.send("dst-reset", "translate", ""),
        },
    )
    .await;
    match r {
        Ok(s) => Ok((s.value.trim().to_string(), s.describe())),
        Err(f) => Err(format!("翻译失败：{}", describe_failures(&f))),
    }
}

fn spawn<R: Runtime>(app: &AppHandle<R>, id: String, fut: impl std::future::Future<Output = ()> + Send + 'static) {
    let handle = tauri::async_runtime::spawn(fut);
    app.state::<TaskState>().replace(&id, handle);
}

/// 截图翻译：裁出选区原图（不带标注，标注会干扰识别）→ 识别 → 翻译。
#[tauri::command]
pub fn capture_translate(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    id: String,
    index: usize,
    region: Region,
) -> Result<(), String> {
    let img = crate::export::crop_base(&app, index, region)?;
    let cfg = state.config.get();
    let out = Emit { win: window, id: id.clone() };
    spawn(&app, id, async move {
        let t0 = Instant::now();
        let png = match tauri::async_runtime::spawn_blocking(move || encode_for_vision(img)).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return out.send("error", "recognize", e),
            Err(e) => return out.send("error", "recognize", e.to_string()),
        };
        out.send("stage", "recognize", "");
        let (source, rec) = match recognize(&cfg, png, &out).await {
            Ok(r) => r,
            Err(e) => return out.send("error", "recognize", e),
        };
        if source.is_empty() {
            return out.send("error", "recognize", "选区里没有识别到文字");
        }
        match translate(&cfg, &source, &out).await {
            Ok((_, tr)) => out.send("done", "translate", format!("{rec} → {tr} · {}", secs(t0))),
            Err(e) => out.send("error", "translate", e),
        }
    });
    Ok(())
}

/// 按（改过的）原文重新翻译。
#[tauri::command]
pub fn translate_text(app: AppHandle, window: WebviewWindow, state: State<'_, AppState>, id: String, text: String) -> Result<(), String> {
    let cfg = state.config.get();
    let out = Emit { win: window, id: id.clone() };
    spawn(&app, id, async move {
        let t0 = Instant::now();
        let source = text.trim().to_string();
        if source.is_empty() {
            return out.send("error", "translate", "原文是空的");
        }
        match translate(&cfg, &source, &out).await {
            Ok((_, tr)) => out.send("done", "translate", format!("{tr} · {}", secs(t0))),
            Err(e) => out.send("error", "translate", e),
        }
    });
    Ok(())
}

#[tauri::command]
pub fn task_cancel(app: AppHandle, id: String) {
    app.state::<TaskState>().cancel(&id);
}

fn encode_for_vision(img: RgbaImage) -> Result<Vec<u8>, String> {
    let (w, h) = fit_size(img.width, img.height, MAX_IMAGE_SIDE);
    let img = if (w, h) == (img.width, img.height) { img } else { export::resize(img, w, h).map_err(|e| e.to_string())? };
    export::encode(&img, ImageFormat::Png, 0, true).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit() {
        assert_eq!(fit_size(800, 600, 2048), (800, 600));
        assert_eq!(fit_size(4096, 1024, 2048), (2048, 512));
        assert_eq!(fit_size(100, 9000, 2048), (23, 2048));
    }
}
