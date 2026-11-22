//! 截图翻译（需求 3.8、M6.1）：识别链 → 翻译链，结果流式推给发起的窗口。
//!
//! 每个任务有前端给的 id；同一个 id 再次发起或调用 `task_cancel` 都会中止旧任务
//! （中止就是丢弃 future，连接随之关闭）。事件 `task:event` 只发给发起任务的窗口。

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use serde::Serialize;
use snapmind_core::config::{BackendRef, Config, SameLang, TranslationConfig};
use snapmind_core::history::{NewTranslation, TransOrigin};
use snapmind_core::lang::{detect, is_same_language, lang_name};
use snapmind_core::export::{self, ImageFormat, RgbaImage};
use snapmind_core::prompts::{parse_oneshot, render, PromptKind, PromptVars};
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
    /// 任务 id → 翻译历史 id：同一张卡片重新翻译时更新原记录，不新增
    records: Mutex<HashMap<String, i64>>,
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
        self.records.lock().expect("任务锁").remove(id);
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

/// 翻译完成后写入翻译历史（需求第 7 节）；同一任务再次完成时更新原记录。历史库不可用时只记日志。
fn record<R: Runtime>(
    app: &AppHandle<R>,
    task: &str,
    source: &str,
    translation: &str,
    target: &str,
    backend: &str,
    origin: TransOrigin,
) {
    let state = app.state::<AppState>();
    let Some(history) = state.history.as_ref() else { return };
    let entry = || NewTranslation {
        source,
        translation,
        source_lang: detect(source).unwrap_or(""),
        target_lang: target,
        backend,
        origin,
        at: now_ms(),
    };
    let tasks = app.state::<TaskState>();
    let existing = tasks.records.lock().expect("任务锁").get(task).copied();
    let result = match existing {
        Some(id) => history.update_translation(id, entry()).map(|_| id),
        None => history.add_translation(entry()),
    };
    match result {
        Ok(id) => {
            tasks.records.lock().expect("任务锁").insert(task.to_string(), id);
        }
        Err(e) => eprintln!("写入翻译历史失败：{e}"),
    }
}

/// 推给前端的事件。`kind`：
/// - `stage`：开始某一步（`stage` = recognize / oneshot / translate，`text` = 正在用的后端）
/// - `src` / `dst`：原文 / 译文的增量；`src-reset` / `dst-reset`：切换后端，清掉半截输出
/// - `done`：完成，`text` = 说明（实际使用的后端、切换原因、耗时）
/// - `error`：失败，`text` = 原因
/// - `overlay`：原位覆盖的段落块（`text` 是 JSON 数组，见 [`OverlayBlock`]）
#[derive(Debug, Clone, Serialize)]
pub struct TaskEvent {
    id: String,
    kind: &'static str,
    stage: &'static str,
    text: String,
}

#[derive(Clone)]
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

/// 渲染提示词用的变量（不翻译列表、术语和格式开关来自翻译偏好）。
fn vars<'a>(t: &'a TranslationConfig, target: &'a str, text: &'a str) -> PromptVars<'a> {
    PromptVars {
        target_lang: target,
        source_lang: None,
        glossary: &t.glossary,
        text,
        no_auto_terms: !t.auto_terms,
        no_keep_format: !t.keep_format,
    }
}

/// 这段原文要翻成什么。
#[derive(Debug, PartialEq)]
pub enum Plan {
    /// 翻成这种语言；第二个值是给用户的说明（换成第二语言时才有）
    Translate(String, Option<String>),
    /// 原文已经是目标语言，按设置不翻译
    Skip,
}

/// 原文已经是目标语言时，按设置换成第二语言或不翻译（只在语言检测有把握时生效）。
pub fn plan(t: &TranslationConfig, source: &str) -> Plan {
    if !is_same_language(source, &t.target_lang) {
        return Plan::Translate(t.target_lang.clone(), None);
    }
    match t.same_lang {
        SameLang::Skip => Plan::Skip,
        SameLang::Second if is_same_language(source, &t.second_lang) => Plan::Skip,
        SameLang::Second => Plan::Translate(
            t.second_lang.clone(),
            Some(format!("原文已是{}，翻译成{}", lang_name(&t.target_lang), lang_name(&t.second_lang))),
        ),
    }
}

/// 识别链：本地 OCR（用原图）或视觉模型（用缩小后的 PNG）。返回（原文，说明）。
async fn recognize<R: Runtime>(
    app: &AppHandle<R>,
    cfg: &Config,
    img: std::sync::Arc<RgbaImage>,
    png: Vec<u8>,
    out: &Emit<R>,
) -> Result<(String, String), String> {
    recognize_as(app, cfg, img, png, out, PromptKind::Recognize).await
}

/// 按识别链识别，`kind` 决定输出：Recognize 是纯文字，Markdown 是 Markdown（视觉模型用 Markdown
/// 提示词，本地 OCR 按版面转换）。事件的 stage 分别是 recognize / markdown。
async fn recognize_as<R: Runtime>(
    app: &AppHandle<R>,
    cfg: &Config,
    img: std::sync::Arc<RgbaImage>,
    png: Vec<u8>,
    out: &Emit<R>,
    kind: PromptKind,
) -> Result<(String, String), String> {
    let markdown = kind == PromptKind::Markdown;
    let stage: &'static str = if markdown { "markdown" } else { "recognize" };
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
    let prompt = render(cfg.prompts.template(kind), kind, &vars(&cfg.translation, &cfg.translation.target_lang, ""));
    let job = TextJob {
        messages: vec![Message::user(vec![Part::Image { mime: "image/png".into(), data: png }, Part::Text(prompt)])],
        temperature: Some(0.0),
        ..Default::default()
    };
    let timeout = chains.timeout();
    let mut on_delta = |d: &str| out.send("src", stage, d);
    let mut run = ChainRun::new(labels, chains.fallback);
    for i in 0..run.len() {
        out.send("stage", stage, run.label(i));
        let r = match &steps[i] {
            // 模型没下载时报不可用，切到下一个
            Step::LocalOcr => {
                let (app, img) = (app.clone(), img.clone());
                let local = move || {
                    if markdown {
                        crate::ocr::recognize_markdown(&app, &img)
                    } else {
                        crate::ocr::recognize_text(&app, &img)
                    }
                };
                match tauri::async_runtime::spawn_blocking(local).await {
                    Ok(Ok(text)) if text.trim().is_empty() => Err(ProviderError::Empty),
                    Ok(Ok(text)) => {
                        out.send("src", stage, text.clone());
                        Ok(text)
                    }
                    Ok(Err(e)) => Err(ProviderError::Unavailable(e)),
                    Err(e) => Err(ProviderError::Unavailable(e.to_string())),
                }
            }
            Step::Llm(b) => b.run_text(&job, timeout, &mut on_delta).await,
        };
        match r {
            Ok(text) => {
                let s = run.succeed(i, text);
                return Ok((s.value.trim().to_string(), s.describe()));
            }
            Err(e) => {
                run.fail(i, e);
                out.send("src-reset", stage, "");
            }
        }
    }
    Err(format!("识别失败：{}", describe_failures(&run.failures())))
}

/// 识别为 Markdown（需求 3.8、M8.1）：按识别链，结果流式推给卡片（src 事件，stage = markdown）。
#[tauri::command]
pub fn capture_markdown(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    id: String,
    index: usize,
    region: Region,
) -> Result<(), String> {
    let img = std::sync::Arc::new(crate::export::crop_base(&app, index, region)?);
    let cfg = state.config.get();
    let out = Emit { win: window, id: id.clone() };
    let handle = app.clone();
    spawn(&app, id, async move {
        let t0 = Instant::now();
        let for_vision = img.clone();
        let png = match tauri::async_runtime::spawn_blocking(move || encode_for_vision(&for_vision)).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return out.send("error", "markdown", e),
            Err(e) => return out.send("error", "markdown", e.to_string()),
        };
        out.send("stage", "markdown", "");
        match recognize_as(&handle, &cfg, img, png, &out, PromptKind::Markdown).await {
            Ok((md, _)) if md.is_empty() => out.send("error", "markdown", "选区里没有识别到文字"),
            Ok((_, desc)) => out.send("done", "markdown", format!("{desc} · {}", secs(t0))),
            Err(e) => out.send("error", "markdown", e),
        }
    });
    Ok(())
}

/// 翻译链。返回（译文，说明，实际目标语言；原文已是目标语言而没有翻译时为 None）。
async fn translate<R: Runtime>(cfg: &Config, source: &str, out: &Emit<R>) -> Result<(String, String, Option<String>), String> {
    let chains = &cfg.chains;
    let backends = resolve(&chains.translate, &cfg.providers, &cfg.mt, key_of);
    if backends.is_empty() {
        return Err("还没有翻译后端：在设置的「备用链 · 翻译」里添加大模型或机器翻译".into());
    }
    let (target, note) = match plan(&cfg.translation, source) {
        Plan::Translate(t, n) => (t, n),
        Plan::Skip => {
            out.send("dst-reset", "translate", "");
            out.send("dst", "translate", source);
            return Ok((source.to_string(), format!("原文已是{}，未翻译", lang_name(&cfg.translation.target_lang)), None));
        }
    };
    let target = target.as_str();
    let prompt = render(cfg.prompts.template(PromptKind::Translate), PromptKind::Translate, &vars(&cfg.translation, target, source));
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
        Ok(s) => {
            let desc = match note {
                Some(n) => format!("{}（{n}）", s.describe()),
                None => s.describe(),
            };
            Ok((s.value.trim().to_string(), desc, Some(target.to_string())))
        }
        Err(f) => Err(format!("翻译失败：{}", describe_failures(&f))),
    }
}

/// 一步到位链：视觉模型看图直接给出原文和译文（JSON）。
/// 返回 `Ok(None)` 表示走到了「降级」或链里没有可用的视觉模型，由调用方改走识别 → 翻译。
async fn oneshot<R: Runtime>(cfg: &Config, png: &[u8], out: &Emit<R>) -> Result<Option<((String, String), String)>, String> {
    let chains = &cfg.chains;
    enum Step {
        Degrade,
        Llm(Backend),
    }
    let steps: Vec<Step> = chains
        .oneshot
        .iter()
        .filter_map(|r| match r {
            BackendRef::Degrade => Some(Step::Degrade),
            other => resolve(std::slice::from_ref(other), &cfg.providers, &cfg.mt, key_of).pop().filter(Backend::is_llm).map(Step::Llm),
        })
        .collect();
    if !steps.iter().any(|s| matches!(s, Step::Llm(_))) {
        return Ok(None);
    }
    let labels: Vec<String> = steps
        .iter()
        .map(|s| match s {
            Step::Degrade => "降级".to_string(),
            Step::Llm(b) => b.label(),
        })
        .collect();
    let t = &cfg.translation;
    let prompt = render(cfg.prompts.template(PromptKind::Oneshot), PromptKind::Oneshot, &vars(t, &t.target_lang, ""));
    let job = TextJob {
        messages: vec![Message::user(vec![Part::Image { mime: "image/png".into(), data: png.to_vec() }, Part::Text(prompt)])],
        temperature: Some(0.0),
        ..Default::default()
    };
    let mut run = ChainRun::new(labels, chains.fallback);
    for i in 0..run.len() {
        let b = match &steps[i] {
            Step::Degrade => return Ok(None),
            Step::Llm(b) => b,
        };
        out.send("stage", "oneshot", run.label(i));
        // 输出是 JSON，半截没法给人看：收完整再解析
        let r = b.run_text(&job, chains.timeout(), &mut |_| {}).await.and_then(|text| {
            parse_oneshot(&text).ok_or_else(|| ProviderError::Parse("没有按要求输出 JSON".into()))
        });
        match r {
            Ok((src, dst)) => {
                let s = run.succeed(i, (src, dst));
                out.send("src", "oneshot", s.value.0.clone());
                out.send("dst", "oneshot", s.value.1.clone());
                let desc = format!("一步到位 {}", s.describe());
                return Ok(Some((s.value, desc)));
            }
            Err(e) => {
                run.fail(i, e);
            }
        }
    }
    Err(format!("一步到位失败：{}", describe_failures(&run.failures())))
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
    let handle = app.clone();
    spawn(&app, id.clone(), async move {
        let t0 = Instant::now();
        let img = std::sync::Arc::new(img);
        let for_vision = img.clone();
        let png = match tauri::async_runtime::spawn_blocking(move || encode_for_vision(&for_vision)).await {
            Ok(Ok(p)) => p,
            Ok(Err(e)) => return out.send("error", "recognize", e),
            Err(e) => return out.send("error", "recognize", e.to_string()),
        };
        let mut prefix = String::new();
        if cfg.translation.oneshot {
            out.send("stage", "oneshot", "");
            match oneshot(&cfg, &png, &out).await {
                Ok(Some((pair, desc))) => {
                    let source = pair.0.clone();
                    // 原文已是目标语言：按设置改翻第二语言或不翻译（会替换掉一步到位给的译文）
                    if plan(&cfg.translation, &source) == Plan::Translate(cfg.translation.target_lang.clone(), None) {
                        let (_, dst) = &pair;
                        record(&handle, &id, &source, dst, &cfg.translation.target_lang, &desc, TransOrigin::Capture);
                        return out.send("done", "oneshot", format!("{desc} · {}", secs(t0)));
                    }
                    out.send("dst-reset", "translate", "");
                    return match translate(&cfg, &source, &out).await {
                        Ok((dst, tr, target)) => {
                            let desc = format!("{desc} → {tr}");
                            if let Some(t) = target {
                                record(&handle, &id, &source, &dst, &t, &desc, TransOrigin::Capture);
                            }
                            out.send("done", "translate", format!("{desc} · {}", secs(t0)))
                        }
                        Err(e) => out.send("error", "translate", e),
                    };
                }
                Ok(None) => prefix = "一步到位已降级 → ".into(),
                Err(e) => return out.send("error", "oneshot", e),
            }
        }
        out.send("stage", "recognize", "");
        let (source, rec) = match recognize(&handle, &cfg, img, png, &out).await {
            Ok(r) => r,
            Err(e) => return out.send("error", "recognize", e),
        };
        if source.is_empty() {
            return out.send("error", "recognize", "选区里没有识别到文字");
        }
        match translate(&cfg, &source, &out).await {
            Ok((dst, tr, target)) => {
                let desc = format!("{prefix}{rec} → {tr}");
                if let Some(t) = target {
                    record(&handle, &id, &source, &dst, &t, &desc, TransOrigin::Capture);
                }
                out.send("done", "translate", format!("{desc} · {}", secs(t0)))
            }
            Err(e) => out.send("error", "translate", e),
        }
    });
    Ok(())
}

/// 原位覆盖的一个段落块（坐标相对选区，截图物理像素）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OverlayBlock {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    fg: [u8; 3],
    bg: [u8; 3],
    size: u32,
    /// 保持原样（公式、代码、只有数字符号），不画覆盖
    keep: bool,
    src: String,
    dst: String,
}

/// 整段译文的段数和原文对不上（大模型合并或拆开了段落）时，逐段重译，每次并发 4 个。
/// 逐段翻译的事件用另一个 id 发出，卡片会忽略，不会搅乱已经显示的整段译文。
async fn translate_each<R: Runtime>(cfg: &Config, texts: &[String], out: &Emit<R>) -> Result<Vec<String>, String> {
    let cfg = std::sync::Arc::new(cfg.clone());
    let mut result = Vec::with_capacity(texts.len());
    for (k, chunk) in texts.chunks(4).enumerate() {
        let handles: Vec<_> = chunk
            .iter()
            .enumerate()
            .map(|(j, t)| {
                let (cfg, t) = (cfg.clone(), t.clone());
                let quiet = Emit { win: out.win.clone(), id: format!("{}#{}", out.id, k * 4 + j) };
                tauri::async_runtime::spawn(async move { translate(&cfg, &t, &quiet).await.map(|r| r.0) })
            })
            .collect();
        for h in handles {
            result.push(h.await.map_err(|e| e.to_string())??);
        }
    }
    Ok(result)
}

/// 截图翻译 · 原位覆盖（需求第 4 节）：坐标只有本地 OCR 能给，所以不管识别链怎么设都用本地 OCR；
/// 要翻译的段落用空行连成一段一次翻译，再按空行拆回各段；公式、代码、纯数字符号的段落保持原样。
#[tauri::command]
pub fn capture_translate_overlay(
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
    let handle = app.clone();
    spawn(&app, id.clone(), async move {
        let t0 = Instant::now();
        out.send("stage", "recognize", "本地 OCR");
        let (w, h) = (img.width, img.height);
        let ocr_app = handle.clone();
        let lines = match tauri::async_runtime::spawn_blocking(move || crate::ocr::recognize(&ocr_app, &img)).await {
            Ok(Ok(l)) => l,
            Ok(Err(e)) if !crate::ocr::installed(&handle) => {
                return out.send("error", "recognize", format!("{e}。原位覆盖要用本地 OCR：在截图里按 Shift+C 打开「识别文字」下载模型，或者切回卡片"))
            }
            Ok(Err(e)) => return out.send("error", "recognize", e),
            Err(e) => return out.send("error", "recognize", e.to_string()),
        };
        let blocks = snapmind_ocr::overlay::blocks(&lines, w, h);
        let todo: Vec<usize> = (0..blocks.len()).filter(|&i| !blocks[i].keep).collect();
        if todo.is_empty() {
            return out.send("error", "recognize", "选区里没有要翻译的文字");
        }
        let texts: Vec<String> = todo.iter().map(|&i| blocks[i].text.clone()).collect();
        let source = texts.join("\n\n");
        out.send("src", "recognize", source.clone());
        let (dst, tr, target) = match translate(&cfg, &source, &out).await {
            Ok(r) => r,
            Err(e) => return out.send("error", "translate", e),
        };
        let parts = match snapmind_ocr::overlay::split_translation(&dst, todo.len()) {
            Some(p) => p,
            None => match translate_each(&cfg, &texts, &out).await {
                Ok(p) => p,
                Err(e) => return out.send("error", "translate", e),
            },
        };
        if let Some(t) = &target {
            record(&handle, &id, &source, &dst, t, &tr, TransOrigin::Capture);
        }
        let mut dst_of = vec![String::new(); blocks.len()];
        for (k, &i) in todo.iter().enumerate() {
            dst_of[i] = parts[k].clone();
        }
        let payload: Vec<OverlayBlock> = blocks
            .into_iter()
            .zip(dst_of)
            .map(|(b, dst)| OverlayBlock {
                x: b.x,
                y: b.y,
                width: b.width,
                height: b.height,
                fg: b.fg,
                bg: b.bg,
                size: b.size,
                keep: b.keep,
                src: b.text,
                dst,
            })
            .collect();
        out.send("overlay", "translate", serde_json::to_string(&payload).unwrap_or_default());
        out.send("done", "translate", format!("本地 OCR → {tr} · {}", secs(t0)))
    });
    Ok(())
}

/// 按（改过的）原文重新翻译。
/// `target`：这次翻成哪种语言（划词卡片固定后可以切换；不给就按设置）；`selection`：来自划词（写进历史的来源）。
#[tauri::command]
pub fn translate_text(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    id: String,
    text: String,
    target: Option<String>,
    selection: Option<bool>,
) -> Result<(), String> {
    let mut cfg = state.config.get();
    if let Some(t) = target.filter(|t| !t.is_empty()) {
        cfg.translation.target_lang = t;
    }
    let origin = if selection == Some(true) { TransOrigin::Selection } else { TransOrigin::Capture };
    let out = Emit { win: window, id: id.clone() };
    let handle = app.clone();
    spawn(&app, id.clone(), async move {
        let t0 = Instant::now();
        let source = text.trim().to_string();
        if source.is_empty() {
            return out.send("error", "translate", "原文是空的");
        }
        match translate(&cfg, &source, &out).await {
            Ok((dst, tr, target)) => {
                if let Some(t) = target {
                    record(&handle, &id, &source, &dst, &t, &tr, origin);
                }
                out.send("done", "translate", format!("{tr} · {}", secs(t0)))
            }
            Err(e) => out.send("error", "translate", e),
        }
    });
    Ok(())
}

#[tauri::command]
pub fn task_cancel(app: AppHandle, id: String) {
    app.state::<TaskState>().cancel(&id);
}

fn encode_for_vision(img: &RgbaImage) -> Result<Vec<u8>, String> {
    let (w, h) = fit_size(img.width, img.height, MAX_IMAGE_SIDE);
    if (w, h) == (img.width, img.height) {
        return export::encode(img, ImageFormat::Png, 0, true).map_err(|e| e.to_string());
    }
    let small = export::resize(img.clone(), w, h).map_err(|e| e.to_string())?;
    export::encode(&small, ImageFormat::Png, 0, true).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plans_for_same_language() {
        let mut t = TranslationConfig::default();
        assert_eq!(plan(&t, "This is a test of the system"), Plan::Translate("zh-Hans".into(), None));
        assert_eq!(plan(&t, "La traduction"), Plan::Translate("zh-Hans".into(), None), "认不出的语言照常翻");
        assert_eq!(
            plan(&t, "你好，世界"),
            Plan::Translate("en".into(), Some("原文已是简体中文，翻译成English".into()))
        );
        t.same_lang = SameLang::Skip;
        assert_eq!(plan(&t, "你好，世界"), Plan::Skip);
        // 第二语言也和原文相同：不翻译
        t.same_lang = SameLang::Second;
        t.target_lang = "en".into();
        t.second_lang = "en".into();
        assert_eq!(plan(&t, "This is a test of the system"), Plan::Skip);
    }

    #[test]
    fn fit() {
        assert_eq!(fit_size(800, 600, 2048), (800, 600));
        assert_eq!(fit_size(4096, 1024, 2048), (2048, 512));
        assert_eq!(fit_size(100, 9000, 2048), (23, 2048));
    }
}
