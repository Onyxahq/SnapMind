//! 备用链执行器（需求 1.2）：按顺序尝试，失败、超时或返回空结果就换下一个，
//! 返回实际使用的后端和之前失败的原因，用来在结果卡片上写「deepseek-chat 超时，已切换到 DeepL」。

use std::time::Duration;

use snapmind_core::i18n::{tr, trf};
use snapmind_core::config::{BackendRef, MtConfig, MtKind, ProviderConfig, ProviderKind};

use crate::chat::{stream_chat, ChatRequest, Endpoint, Message};
use crate::error::ProviderError;
use crate::mt::{translate, MtEndpoint};

/// 解析好、可以直接调用的后端（地址、Key 都已经从设置和凭据库取出）。
#[derive(Debug, Clone)]
pub enum Backend {
    Llm { kind: ProviderKind, base_url: String, key: Option<String>, model: String },
    Mt { kind: MtKind, key: String, region: Option<String>, base_url: Option<String> },
}

pub fn mt_name(kind: MtKind) -> String {
    match kind {
        MtKind::Deepl => "DeepL".to_string(),
        MtKind::Google => tr("Google 翻译"),
        MtKind::Microsoft => tr("微软翻译"),
    }
}

impl Backend {
    /// 给用户看的名字：大模型写模型名，机器翻译写服务名。
    pub fn label(&self) -> String {
        match self {
            Backend::Llm { model, .. } => model.clone(),
            Backend::Mt { kind, .. } => mt_name(*kind).to_string(),
        }
    }

    pub fn is_llm(&self) -> bool {
        matches!(self, Backend::Llm { .. })
    }

    /// 执行一次文本任务。大模型流式输出；机器翻译一次性交出全文。
    pub async fn run_text(&self, job: &TextJob, timeout: Duration, on_delta: &mut (dyn FnMut(&str) + Send)) -> Result<String, ProviderError> {
        match self {
            Backend::Llm { kind, base_url, key, model } => {
                let ep = Endpoint { kind: *kind, base_url, key: key.as_deref() };
                let mut req = ChatRequest::new(model.clone(), job.messages.clone());
                req.system = job.system.clone();
                req.temperature = job.temperature;
                req.first_token_timeout = timeout;
                stream_chat(&ep, &req, on_delta).await
            }
            Backend::Mt { kind, key, region, base_url } => {
                let ep = MtEndpoint { kind: *kind, key, region: region.as_deref(), base_url: base_url.as_deref() };
                let out = translate(&ep, &job.text, job.source.as_deref(), &job.target, timeout).await?;
                on_delta(&out.text);
                Ok(out.text)
            }
        }
    }
}

/// 把设置里的链（引用）换成可以调用的后端：Key 用 `key_of(账户名)` 从凭据库取。
///
/// - 服务商已移除、模型已取消勾选的项跳过（设置页会把它们标成「已失效」）；
/// - 缺 Key 的项保留，调用时报「没有 Key」并切到下一个，结果卡片能说明原因；
/// - 本地 OCR、降级这两种不是文本后端，由调用方单独处理，这里跳过。
pub fn resolve(refs: &[BackendRef], providers: &[ProviderConfig], mt: &MtConfig, key_of: impl Fn(&str) -> Option<String>) -> Vec<Backend> {
    refs.iter()
        .filter_map(|r| match r {
            BackendRef::Llm { provider, model } => {
                let p = providers.iter().find(|p| &p.id == provider)?;
                p.models.iter().find(|m| &m.id == model && m.enabled)?;
                Some(Backend::Llm { kind: p.kind, base_url: p.base_url.clone(), key: key_of(&p.id), model: model.clone() })
            }
            BackendRef::Mt { kind } => Some(Backend::Mt {
                kind: *kind,
                key: key_of(kind.secret_account()).unwrap_or_default(),
                region: Some(mt.microsoft_region.clone()).filter(|r| !r.trim().is_empty()),
                base_url: None,
            }),
            BackendRef::LocalOcr | BackendRef::Degrade => None,
        })
        .collect()
}

/// 一次文本任务的两种说法：给大模型的提示词，和给机器翻译的原文与语言。
#[derive(Debug, Clone, Default)]
pub struct TextJob {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    /// 机器翻译用：原文
    pub text: String,
    /// 机器翻译用：原文语言（None 自动检测）
    pub source: Option<String>,
    /// 机器翻译用：目标语言（统一写法，如 zh-Hans）
    pub target: String,
}

/// 链中失败的一项。
#[derive(Debug, Clone, PartialEq)]
pub struct Failure {
    pub index: usize,
    pub label: String,
    pub error: ProviderError,
}

/// 执行过程中的事件，界面用来显示「正在用谁」和在切换时清掉半截输出。
#[derive(Debug, PartialEq)]
pub enum ChainEvent<'a> {
    Start { index: usize, label: &'a str },
    Failed(&'a Failure),
}

#[derive(Debug, PartialEq)]
pub struct ChainSuccess<T> {
    pub value: T,
    pub index: usize,
    pub label: String,
    pub failures: Vec<Failure>,
}

impl<T> ChainSuccess<T> {
    /// 「deepseek-chat 超时，已切换到 DeepL」；没有切换时就是后端名。
    pub fn describe(&self) -> String {
        if self.failures.is_empty() {
            return self.label.clone();
        }
        let why: Vec<String> = self.failures.iter().map(|f| format!("{} {}", f.label, f.error.short())).collect();
        trf("{why}，已切换到 {label}", &[("why", &why.join(&tr("，"))), ("label", &self.label)])
    }
}

/// 全部失败时的说明：「deepseek-chat 超时；DeepL 出错（HTTP 500）」。
pub fn describe_failures(failures: &[Failure]) -> String {
    if failures.is_empty() {
        return tr("还没有设置可用的后端");
    }
    failures.iter().map(|f| format!("{} {}", f.label, f.error.short())).collect::<Vec<_>>().join(&tr("；"))
}

/// 一次链式执行的记录。调用方自己写循环（`for i in 0..run.len()`），每项成功就 `succeed`、
/// 失败就 `fail`；这样每一步的 `.await` 都写在普通 async 函数里，
/// 不用把借用了回调的异步闭包传来传去（那样的 future 很难满足跨线程的 Send 要求）。
#[derive(Debug)]
pub struct ChainRun {
    labels: Vec<String>,
    n: usize,
    failures: Vec<Failure>,
}

impl ChainRun {
    /// `fallback` 关掉时只试第 1 项。
    pub fn new(labels: Vec<String>, fallback: bool) -> Self {
        let n = if fallback { labels.len() } else { labels.len().min(1) };
        Self { labels, n, failures: Vec::new() }
    }

    /// 要尝试的项数。
    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn label(&self, index: usize) -> &str {
        &self.labels[index]
    }

    pub fn start(&self, index: usize) -> ChainEvent<'_> {
        ChainEvent::Start { index, label: &self.labels[index] }
    }

    /// 记下第 `index` 项失败，返回这条记录（用来发 `Failed` 事件）。
    pub fn fail(&mut self, index: usize, error: ProviderError) -> &Failure {
        self.failures.push(Failure { index, label: self.labels[index].clone(), error });
        self.failures.last().expect("刚放进去")
    }

    pub fn succeed<T>(self, index: usize, value: T) -> ChainSuccess<T> {
        ChainSuccess { value, index, label: self.labels[index].clone(), failures: self.failures }
    }

    pub fn failures(self) -> Vec<Failure> {
        self.failures
    }
}

/// 文本任务走备用链：大模型和机器翻译可以混排。
pub async fn run_text_chain(
    backends: &[Backend],
    fallback: bool,
    job: &TextJob,
    timeout: Duration,
    mut on_delta: impl FnMut(&str) + Send,
    on_event: impl FnMut(ChainEvent<'_>),
) -> Result<ChainSuccess<String>, Vec<Failure>> {
    let mut on_event = on_event;
    let mut run = ChainRun::new(backends.iter().map(Backend::label).collect(), fallback);
    for i in 0..run.len() {
        on_event(run.start(i));
        match backends[i].run_text(job, timeout, &mut on_delta).await {
            Ok(value) => return Ok(run.succeed(i, value)),
            Err(e) => on_event(ChainEvent::Failed(run.fail(i, e))),
        }
    }
    Err(run.failures())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sse(body: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(body.as_bytes().to_vec(), "text/event-stream")
    }

    fn llm(server: &MockServer, sub: &str, model: &str) -> Backend {
        Backend::Llm { kind: ProviderKind::Openai, base_url: format!("{}/{sub}", server.uri()), key: Some("k".into()), model: model.into() }
    }

    fn mt(server: &MockServer, sub: &str, kind: MtKind) -> Backend {
        Backend::Mt { kind, key: "k".into(), region: None, base_url: Some(format!("{}/{sub}", server.uri())) }
    }

    fn job() -> TextJob {
        TextJob { messages: vec![Message::user_text("Hello")], text: "Hello".into(), target: "zh-Hans".into(), ..Default::default() }
    }

    /// 验收：超时、报错、空结果三种情况都能正确切换，并返回实际使用的后端。
    #[tokio::test]
    async fn switches_on_timeout_error_and_empty() {
        let server = MockServer::start().await;
        Mock::given(path("/slow/chat/completions"))
            .respond_with(sse("data: {\"choices\":[{\"delta\":{\"content\":\"迟到\"}}]}\n\n").set_delay(Duration::from_secs(3)))
            .mount(&server)
            .await;
        Mock::given(path("/dl/v2/translate")).respond_with(ResponseTemplate::new(500).set_body_string("boom")).mount(&server).await;
        Mock::given(path("/empty/chat/completions"))
            .respond_with(sse("data: {\"choices\":[{\"delta\":{\"content\":\"\\n\"}}]}\n\ndata: [DONE]\n\n"))
            .mount(&server)
            .await;
        Mock::given(path("/gg/language/translate/v2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": { "translations": [{ "translatedText": "你好" }] } })))
            .mount(&server)
            .await;

        let chain = vec![
            llm(&server, "slow", "deepseek-chat"),
            mt(&server, "dl", MtKind::Deepl),
            llm(&server, "empty", "claude-x"),
            mt(&server, "gg", MtKind::Google),
        ];
        let mut deltas = Vec::new();
        let mut events = Vec::new();
        let t0 = std::time::Instant::now();
        let r = run_text_chain(
            &chain,
            true,
            &job(),
            Duration::from_millis(400),
            |d| deltas.push(d.to_string()),
            |e| {
                events.push(match e {
                    ChainEvent::Start { index, .. } => format!("start{index}"),
                    ChainEvent::Failed(f) => format!("fail{}", f.index),
                })
            },
        )
        .await
        .unwrap();
        assert!(t0.elapsed() < Duration::from_secs(2), "第一项按超时放弃，不等它的 3 秒");
        assert_eq!(r.value, "你好");
        assert_eq!((r.index, r.label.as_str()), (3, "Google 翻译"));
        let errs: Vec<&ProviderError> = r.failures.iter().map(|f| &f.error).collect();
        assert_eq!(errs, vec![&ProviderError::Timeout, &ProviderError::Http { status: 500, message: "boom".into() }, &ProviderError::Empty]);
        assert_eq!(events, vec!["start0", "fail0", "start1", "fail1", "start2", "fail2", "start3"]);
        assert_eq!(deltas.last().map(String::as_str), Some("你好"));
        assert_eq!(r.describe(), "deepseek-chat 超时，DeepL 出错（HTTP 500），claude-x 返回空结果，已切换到 Google 翻译");
    }

    #[tokio::test]
    async fn first_success_stops_and_fallback_off_tries_only_first() {
        let server = MockServer::start().await;
        Mock::given(path("/ok/chat/completions"))
            .respond_with(sse("data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\ndata: [DONE]\n\n"))
            .expect(1)
            .mount(&server)
            .await;
        Mock::given(path("/bad/chat/completions")).respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(path("/never/chat/completions")).respond_with(ResponseTemplate::new(200)).expect(0).mount(&server).await;

        let ok = run_text_chain(&[llm(&server, "ok", "a"), llm(&server, "never", "b")], true, &job(), Duration::from_secs(5), |_| {}, |_| {})
            .await
            .unwrap();
        assert_eq!((ok.value.as_str(), ok.describe()), ("好", "a".to_string()));

        let off = run_text_chain(&[llm(&server, "bad", "a"), llm(&server, "never", "b")], false, &job(), Duration::from_secs(5), |_| {}, |_| {})
            .await
            .unwrap_err();
        assert_eq!(off, vec![Failure { index: 0, label: "a".into(), error: ProviderError::Auth(401) }]);
        assert_eq!(describe_failures(&off), "a Key 无效");
    }

    #[tokio::test]
    async fn all_fail_and_empty_chain() {
        let server = MockServer::start().await;
        Mock::given(path("/x/chat/completions")).respond_with(ResponseTemplate::new(503)).mount(&server).await;
        let chain = vec![llm(&server, "x", "a"), Backend::Mt { kind: MtKind::Microsoft, key: " ".into(), region: None, base_url: None }];
        let e = run_text_chain(&chain, true, &job(), Duration::from_secs(5), |_| {}, |_| {}).await.unwrap_err();
        assert_eq!(describe_failures(&e), "a 出错（HTTP 503）；微软翻译 没有 Key");
        let none = run_text_chain(&[], true, &job(), Duration::from_secs(5), |_| {}, |_| {}).await.unwrap_err();
        assert!(none.is_empty());
        assert_eq!(describe_failures(&none), "还没有设置可用的后端");
    }

    #[test]
    fn resolve_skips_stale_refs_and_keeps_missing_keys() {
        use snapmind_core::config::ModelConfig;
        let providers = vec![ProviderConfig {
            id: "deepseek".into(),
            kind: ProviderKind::Deepseek,
            name: "DeepSeek".into(),
            base_url: "https://api.deepseek.com/v1".into(),
            models: vec![
                ModelConfig { id: "deepseek-chat".into(), tags: vec![], enabled: true },
                ModelConfig { id: "deepseek-reasoner".into(), tags: vec![], enabled: false },
            ],
        }];
        let refs = vec![
            BackendRef::Llm { provider: "deepseek".into(), model: "deepseek-chat".into() },
            BackendRef::Llm { provider: "deepseek".into(), model: "deepseek-reasoner".into() },
            BackendRef::Llm { provider: "gone".into(), model: "x".into() },
            BackendRef::Mt { kind: MtKind::Microsoft },
            BackendRef::Degrade,
        ];
        let mt = MtConfig { microsoft_region: "eastasia".into() };
        let got = resolve(&refs, &providers, &mt, |acct| (acct == "deepseek").then(|| "sk".to_string()));
        assert_eq!(got.len(), 2);
        assert!(matches!(&got[0], Backend::Llm { key: Some(k), model, .. } if k == "sk" && model == "deepseek-chat"));
        assert!(matches!(&got[1], Backend::Mt { key, region: Some(r), .. } if key.is_empty() && r == "eastasia"));
    }

    #[tokio::test]
    async fn chain_run_records_failures() {
        let labels: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let mut run = ChainRun::new(labels.clone(), true);
        assert_eq!(run.len(), 3);
        run.fail(0, ProviderError::Empty);
        run.fail(1, ProviderError::Timeout);
        let ok = run.succeed(2, 20);
        assert_eq!((ok.value, ok.label.as_str(), ok.failures.len()), (20, "c", 2));
        assert_eq!(ok.describe(), "a 返回空结果，b 超时，已切换到 c");
        assert_eq!(ChainRun::new(labels, false).len(), 1);
        assert!(ChainRun::new(vec![], true).is_empty());
    }
}
