//! 流式对话：OpenAI 兼容 / Claude 原生 / Gemini 原生三种接口，支持文本和图片输入。
//!
//! 调用方式统一为 [`stream_chat`]：边收边通过回调交出增量文字，结束后返回全文。
//! 取消 = 丢弃这个 future（连接随之关闭）。

use std::time::Duration;

use base64::Engine as _;
use serde_json::{json, Value};
use snapmind_core::config::ProviderKind;
use tokio::time::{timeout, timeout_at, Instant};

use crate::catalog::{meta, ApiFormat};
use crate::error::{check, net, ProviderError};
use crate::sse::{SseEvent, SseParser};

/// 要调用的服务（地址和 Key 来自设置里的服务商）。
#[derive(Debug, Clone, Copy)]
pub struct Endpoint<'a> {
    pub kind: ProviderKind,
    pub base_url: &'a str,
    pub key: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Text(String),
    /// 图片字节和类型（如 `image/png`），发送时转 base64
    Image { mime: String, data: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub role: Role,
    pub parts: Vec<Part>,
}

impl Message {
    pub fn user(parts: Vec<Part>) -> Self {
        Self { role: Role::User, parts }
    }
    pub fn user_text(text: impl Into<String>) -> Self {
        Self::user(vec![Part::Text(text.into())])
    }
}

#[derive(Debug, Clone)]
pub struct ChatRequest {
    pub model: String,
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub temperature: Option<f32>,
    /// Claude 必填，默认 4096；其他接口不填就用服务端默认
    pub max_tokens: Option<u32>,
    /// 从发出请求到收到第一段文字的最长等待（备用链的「超时」）
    pub first_token_timeout: Duration,
    /// 开始输出后，两段之间的最长间隔
    pub idle_timeout: Duration,
}

impl ChatRequest {
    pub fn new(model: impl Into<String>, messages: Vec<Message>) -> Self {
        Self {
            model: model.into(),
            system: None,
            messages,
            temperature: None,
            max_tokens: None,
            first_token_timeout: Duration::from_secs(30),
            idle_timeout: Duration::from_secs(30),
        }
    }
}

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const CLAUDE_DEFAULT_MAX_TOKENS: u32 = 4096;

fn b64(data: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn join(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
}

// ---------------------------------------------------------------- 请求体

fn openai_body(req: &ChatRequest) -> Value {
    let mut messages = Vec::new();
    if let Some(s) = &req.system {
        messages.push(json!({ "role": "system", "content": s }));
    }
    for m in &req.messages {
        let role = if m.role == Role::User { "user" } else { "assistant" };
        // 只有一段文字时用字符串，兼容不支持数组内容的老接口
        let content = match m.parts.as_slice() {
            [Part::Text(t)] => json!(t),
            parts => Value::Array(
                parts
                    .iter()
                    .map(|p| match p {
                        Part::Text(t) => json!({ "type": "text", "text": t }),
                        Part::Image { mime, data } => json!({
                            "type": "image_url",
                            "image_url": { "url": format!("data:{mime};base64,{}", b64(data)) }
                        }),
                    })
                    .collect(),
            ),
        };
        messages.push(json!({ "role": role, "content": content }));
    }
    let mut body = json!({ "model": req.model, "messages": messages, "stream": true });
    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }
    if let Some(n) = req.max_tokens {
        body["max_tokens"] = json!(n);
    }
    body
}

fn claude_body(req: &ChatRequest) -> Value {
    let messages: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            let content: Vec<Value> = m
                .parts
                .iter()
                .map(|p| match p {
                    Part::Text(t) => json!({ "type": "text", "text": t }),
                    Part::Image { mime, data } => json!({
                        "type": "image",
                        "source": { "type": "base64", "media_type": mime, "data": b64(data) }
                    }),
                })
                .collect();
            json!({ "role": if m.role == Role::User { "user" } else { "assistant" }, "content": content })
        })
        .collect();
    let mut body = json!({
        "model": req.model,
        "messages": messages,
        "max_tokens": req.max_tokens.unwrap_or(CLAUDE_DEFAULT_MAX_TOKENS),
        "stream": true,
    });
    if let Some(s) = &req.system {
        body["system"] = json!(s);
    }
    if let Some(t) = req.temperature {
        body["temperature"] = json!(t);
    }
    body
}

fn gemini_body(req: &ChatRequest) -> Value {
    let contents: Vec<Value> = req
        .messages
        .iter()
        .map(|m| {
            let parts: Vec<Value> = m
                .parts
                .iter()
                .map(|p| match p {
                    Part::Text(t) => json!({ "text": t }),
                    Part::Image { mime, data } => json!({ "inlineData": { "mimeType": mime, "data": b64(data) } }),
                })
                .collect();
            json!({ "role": if m.role == Role::User { "user" } else { "model" }, "parts": parts })
        })
        .collect();
    let mut body = json!({ "contents": contents });
    if let Some(s) = &req.system {
        body["systemInstruction"] = json!({ "parts": [{ "text": s }] });
    }
    let mut gen = serde_json::Map::new();
    if let Some(t) = req.temperature {
        gen.insert("temperature".into(), json!(t));
    }
    if let Some(n) = req.max_tokens {
        gen.insert("maxOutputTokens".into(), json!(n));
    }
    if !gen.is_empty() {
        body["generationConfig"] = Value::Object(gen);
    }
    body
}

// ---------------------------------------------------------------- 流式事件

/// 一个 SSE 事件的含义。
#[derive(Debug, PartialEq)]
enum Step {
    Text(String),
    Done,
    Skip,
}

fn parse_json(data: &str) -> Result<Value, ProviderError> {
    serde_json::from_str(data).map_err(|e| ProviderError::Parse(format!("{e}：{}", data.chars().take(120).collect::<String>())))
}

fn remote_error(v: &Value) -> Option<ProviderError> {
    let e = v.get("error")?;
    let msg = e.get("message").and_then(Value::as_str).map(str::to_string).unwrap_or_else(|| e.to_string());
    Some(ProviderError::Remote(msg))
}

fn openai_step(ev: &SseEvent) -> Result<Step, ProviderError> {
    if ev.data.trim() == "[DONE]" {
        return Ok(Step::Done);
    }
    let v = parse_json(&ev.data)?;
    if let Some(e) = remote_error(&v) {
        return Err(e);
    }
    let choice = v.pointer("/choices/0");
    if choice.and_then(|c| c.get("finish_reason")).and_then(Value::as_str) == Some("content_filter") {
        return Err(ProviderError::Refused("触发了内容过滤".into()));
    }
    // DeepSeek 等推理模型的 reasoning_content 是思考过程，不算输出
    match choice.and_then(|c| c.pointer("/delta/content")).and_then(Value::as_str) {
        Some(t) if !t.is_empty() => Ok(Step::Text(t.to_string())),
        _ => Ok(Step::Skip),
    }
}

fn claude_step(ev: &SseEvent) -> Result<Step, ProviderError> {
    let v = parse_json(&ev.data)?;
    let kind = if ev.event.is_empty() { v.get("type").and_then(Value::as_str).unwrap_or("") } else { ev.event.as_str() };
    match kind {
        "error" => Err(remote_error(&v).unwrap_or_else(|| ProviderError::Remote(ev.data.clone()))),
        "message_stop" => Ok(Step::Done),
        "content_block_delta" if v.pointer("/delta/type").and_then(Value::as_str) == Some("text_delta") => {
            Ok(Step::Text(v.pointer("/delta/text").and_then(Value::as_str).unwrap_or("").to_string()))
        }
        "message_delta" if v.pointer("/delta/stop_reason").and_then(Value::as_str) == Some("refusal") => {
            Err(ProviderError::Refused("模型拒绝回答".into()))
        }
        _ => Ok(Step::Skip),
    }
}

fn gemini_step(ev: &SseEvent) -> Result<Step, ProviderError> {
    let v = parse_json(&ev.data)?;
    if let Some(e) = remote_error(&v) {
        return Err(e);
    }
    if let Some(r) = v.pointer("/promptFeedback/blockReason").and_then(Value::as_str) {
        return Err(ProviderError::Refused(r.to_string()));
    }
    let cand = v.pointer("/candidates/0");
    let text: String = cand
        .and_then(|c| c.pointer("/content/parts"))
        .and_then(Value::as_array)
        .map(|parts| {
            parts
                .iter()
                .filter(|p| p.get("thought").and_then(Value::as_bool) != Some(true))
                .filter_map(|p| p.get("text").and_then(Value::as_str))
                .collect()
        })
        .unwrap_or_default();
    if text.is_empty() {
        let reason = cand.and_then(|c| c.get("finishReason")).and_then(Value::as_str).unwrap_or("");
        if matches!(reason, "SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "SPII" | "RECITATION") {
            return Err(ProviderError::Refused(reason.to_string()));
        }
        return Ok(Step::Skip);
    }
    Ok(Step::Text(text))
}

// ---------------------------------------------------------------- 入口

/// 发起流式对话。`on_delta` 收到每一段新文字；返回完整文字。
/// 结果去掉空白后为空时返回 [`ProviderError::Empty`]，方便备用链切换。
pub async fn stream_chat(
    ep: &Endpoint<'_>,
    req: &ChatRequest,
    mut on_delta: impl FnMut(&str),
) -> Result<String, ProviderError> {
    let m = meta(ep.kind);
    let key = ep.key.map(str::trim).filter(|k| !k.is_empty());
    if m.needs_key && key.is_none() {
        return Err(ProviderError::MissingKey);
    }
    let client = crate::net::client(reqwest::Client::builder().connect_timeout(CONNECT_TIMEOUT)).await.map_err(net)?;
    let builder = match m.format {
        ApiFormat::OpenAi => {
            let r = client.post(join(ep.base_url, "chat/completions")).json(&openai_body(req));
            match key {
                Some(k) => r.bearer_auth(k),
                None => r,
            }
        }
        ApiFormat::Claude => client
            .post(join(ep.base_url, "v1/messages"))
            .header("x-api-key", key.unwrap_or(""))
            .header("anthropic-version", "2023-06-01")
            .json(&claude_body(req)),
        ApiFormat::Gemini => {
            let model = req.model.trim_start_matches("models/");
            client
                .post(join(ep.base_url, &format!("v1beta/models/{model}:streamGenerateContent?alt=sse")))
                .header("x-goog-api-key", key.unwrap_or(""))
                .json(&gemini_body(req))
        }
    };
    let step = match m.format {
        ApiFormat::OpenAi => openai_step,
        ApiFormat::Claude => claude_step,
        ApiFormat::Gemini => gemini_step,
    };

    let first_deadline = Instant::now() + req.first_token_timeout;
    let resp = timeout_at(first_deadline, builder.header("accept", "text/event-stream").send())
        .await
        .map_err(|_| ProviderError::Timeout)?
        .map_err(net)?;
    let mut resp = check(resp).await?;

    let mut parser = SseParser::default();
    let mut text = String::new();
    'read: loop {
        let next = resp.chunk();
        let chunk = if text.is_empty() {
            timeout_at(first_deadline, next).await
        } else {
            timeout(req.idle_timeout, next).await
        }
        .map_err(|_| ProviderError::Timeout)?
        .map_err(net)?;
        let ended = chunk.is_none();
        let events = match chunk {
            Some(bytes) => parser.push(&bytes),
            None => parser.finish(),
        };
        for ev in &events {
            match step(ev)? {
                Step::Text(t) => {
                    on_delta(&t);
                    text.push_str(&t);
                }
                Step::Done => break 'read,
                Step::Skip => {}
            }
        }
        if ended {
            break;
        }
    }
    if text.trim().is_empty() {
        return Err(ProviderError::Empty);
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_partial_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn sse(body: &str) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(body.as_bytes().to_vec(), "text/event-stream")
    }

    fn png() -> Part {
        Part::Image { mime: "image/png".into(), data: vec![1, 2, 3] }
    }

    async fn run(ep: Endpoint<'_>, req: &ChatRequest) -> (Result<String, ProviderError>, Vec<String>) {
        let mut deltas = Vec::new();
        let r = stream_chat(&ep, req, |d| deltas.push(d.to_string())).await;
        (r, deltas)
    }

    #[tokio::test]
    async fn openai_streams_text_and_sends_images_as_data_urls() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer sk"))
            .and(body_partial_json(json!({
                "model": "gpt-x", "stream": true, "temperature": 0.5,
                "messages": [
                    { "role": "system", "content": "sys" },
                    { "role": "user", "content": [
                        { "type": "text", "text": "看图" },
                        { "type": "image_url", "image_url": { "url": "data:image/png;base64,AQID" } }
                    ]}
                ]
            })))
            .respond_with(sse(concat!(
                "data: {\"choices\":[{\"delta\":{\"role\":\"assistant\",\"content\":\"\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"reasoning_content\":\"思考\"}}]}\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
                ": keep-alive\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"好\"},\"finish_reason\":\"stop\"}]}\n\n",
                "data: [DONE]\n\n",
            )))
            .mount(&server)
            .await;
        let base = format!("{}/v1", server.uri());
        let ep = Endpoint { kind: ProviderKind::Openai, base_url: &base, key: Some("sk") };
        let mut req = ChatRequest::new("gpt-x", vec![Message::user(vec![Part::Text("看图".into()), png()])]);
        req.system = Some("sys".into());
        req.temperature = Some(0.5);
        let (r, d) = run(ep, &req).await;
        assert_eq!(r.unwrap(), "你好");
        assert_eq!(d, vec!["你", "好"]);
    }

    #[tokio::test]
    async fn openai_single_text_uses_string_content_and_ollama_needs_no_key() {
        let server = MockServer::start().await;
        Mock::given(path("/v1/chat/completions"))
            .and(body_partial_json(json!({ "messages": [{ "role": "user", "content": "hi" }] })))
            .respond_with(sse("data: {\"choices\":[{\"delta\":{\"content\":\"ok\"}}]}\n\ndata: [DONE]\n\n"))
            .mount(&server)
            .await;
        let base = format!("{}/v1", server.uri());
        let ep = Endpoint { kind: ProviderKind::Ollama, base_url: &base, key: None };
        assert_eq!(run(ep, &ChatRequest::new("qwen", vec![Message::user_text("hi")])).await.0.unwrap(), "ok");
    }

    #[tokio::test]
    async fn claude_streams_with_system_and_base64_image() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", "ak"))
            .and(header("anthropic-version", "2023-06-01"))
            .and(body_partial_json(json!({
                "model": "claude-x", "system": "sys", "max_tokens": 4096, "stream": true,
                "messages": [{ "role": "user", "content": [
                    { "type": "image", "source": { "type": "base64", "media_type": "image/png", "data": "AQID" } },
                    { "type": "text", "text": "识别" }
                ]}]
            })))
            .respond_with(sse(concat!(
                "event: message_start\ndata: {\"type\":\"message_start\",\"message\":{}}\n\n",
                "event: content_block_start\ndata: {\"type\":\"content_block_start\",\"index\":0}\n\n",
                "event: ping\ndata: {\"type\":\"ping\"}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"嗯\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\"Hello\"}}\n\n",
                "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"type\":\"text_delta\",\"text\":\" world\"}}\n\n",
                "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
            )))
            .mount(&server)
            .await;
        let base = server.uri();
        let ep = Endpoint { kind: ProviderKind::Anthropic, base_url: &base, key: Some("ak") };
        let mut req = ChatRequest::new("claude-x", vec![Message::user(vec![png(), Part::Text("识别".into())])]);
        req.system = Some("sys".into());
        let (r, d) = run(ep, &req).await;
        assert_eq!(r.unwrap(), "Hello world");
        assert_eq!(d, vec!["Hello", " world"]);
    }

    #[tokio::test]
    async fn gemini_streams_and_maps_roles_and_inline_data() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1beta/models/gemini-x:streamGenerateContent"))
            .and(query_param("alt", "sse"))
            .and(header("x-goog-api-key", "gk"))
            .and(body_partial_json(json!({
                "systemInstruction": { "parts": [{ "text": "sys" }] },
                "contents": [
                    { "role": "user", "parts": [{ "text": "q" }, { "inlineData": { "mimeType": "image/png", "data": "AQID" } }] },
                    { "role": "model", "parts": [{ "text": "a" }] },
                    { "role": "user", "parts": [{ "text": "q2" }] }
                ],
                "generationConfig": { "maxOutputTokens": 100 }
            })))
            .respond_with(sse(concat!(
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"想\",\"thought\":true},{\"text\":\"一\"}],\"role\":\"model\"}}]}\r\n\r\n",
                "data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"二\"}]},\"finishReason\":\"STOP\"}]}\r\n\r\n",
            )))
            .mount(&server)
            .await;
        let base = server.uri();
        let ep = Endpoint { kind: ProviderKind::Gemini, base_url: &base, key: Some("gk") };
        let mut req = ChatRequest::new(
            "models/gemini-x",
            vec![
                Message::user(vec![Part::Text("q".into()), png()]),
                Message { role: Role::Assistant, parts: vec![Part::Text("a".into())] },
                Message::user_text("q2"),
            ],
        );
        req.system = Some("sys".into());
        req.max_tokens = Some(100);
        let (r, d) = run(ep, &req).await;
        assert_eq!(r.unwrap(), "一二");
        assert_eq!(d, vec!["一", "二"]);
    }

    #[tokio::test]
    async fn errors_empty_refusal_and_timeouts() {
        let server = MockServer::start().await;
        let mount = |p: &'static str, t: ResponseTemplate| Mock::given(path(p)).respond_with(t);
        mount("/auth/chat/completions", ResponseTemplate::new(401)).mount(&server).await;
        mount(
            "/busy/chat/completions",
            ResponseTemplate::new(429).set_body_json(json!({ "error": { "message": "slow down" } })),
        )
        .mount(&server)
        .await;
        mount("/midway/chat/completions", sse("data: {\"choices\":[{\"delta\":{\"content\":\"半\"}}]}\n\ndata: {\"error\":{\"message\":\"overloaded\"}}\n\n"))
            .mount(&server)
            .await;
        mount("/empty/chat/completions", sse("data: {\"choices\":[{\"delta\":{\"content\":\"  \"}}]}\n\ndata: [DONE]\n\n")).mount(&server).await;
        mount("/filter/chat/completions", sse("data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"content_filter\"}]}\n\n")).mount(&server).await;
        mount("/slow/chat/completions", sse("data: [DONE]\n\n").set_delay(Duration::from_secs(3))).mount(&server).await;
        mount("/v1beta/models/g:streamGenerateContent", sse("data: {\"promptFeedback\":{\"blockReason\":\"SAFETY\"}}\n\n"))
            .mount(&server)
            .await;
        mount("/v1/messages", sse("event: error\ndata: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n"))
            .mount(&server)
            .await;

        let req = ChatRequest::new("m", vec![Message::user_text("x")]);
        let call = |sub: &'static str, req: ChatRequest| {
            let base = format!("{}/{sub}", server.uri());
            async move { stream_chat(&Endpoint { kind: ProviderKind::Openai, base_url: &base, key: Some("k") }, &req, |_| {}).await }
        };
        assert_eq!(call("auth", req.clone()).await, Err(ProviderError::Auth(401)));
        assert_eq!(call("busy", req.clone()).await, Err(ProviderError::Http { status: 429, message: "slow down".into() }));
        assert_eq!(call("midway", req.clone()).await, Err(ProviderError::Remote("overloaded".into())));
        assert_eq!(call("empty", req.clone()).await, Err(ProviderError::Empty));
        assert!(matches!(call("filter", req.clone()).await, Err(ProviderError::Refused(_))));
        let mut quick = req.clone();
        quick.first_token_timeout = Duration::from_millis(300);
        let t0 = std::time::Instant::now();
        assert_eq!(call("slow", quick).await, Err(ProviderError::Timeout));
        assert!(t0.elapsed() < Duration::from_secs(2), "超时要按时返回");
        assert_eq!(
            stream_chat(&Endpoint { kind: ProviderKind::Openai, base_url: "http://x", key: None }, &req, |_| {}).await,
            Err(ProviderError::MissingKey)
        );

        let base = server.uri();
        let g = stream_chat(&Endpoint { kind: ProviderKind::Gemini, base_url: &base, key: Some("k") }, &ChatRequest::new("g", vec![Message::user_text("x")]), |_| {}).await;
        assert_eq!(g, Err(ProviderError::Refused("SAFETY".into())));
        let c = stream_chat(&Endpoint { kind: ProviderKind::Anthropic, base_url: &base, key: Some("k") }, &req, |_| {}).await;
        assert_eq!(c, Err(ProviderError::Remote("Overloaded".into())));
    }

    #[tokio::test]
    async fn idle_timeout_after_first_token() {
        // 先给一段文字然后不再结束：用一个只发一半就停的本地服务器模拟
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut s, _) = listener.accept().await.unwrap();
            let mut buf = [0u8; 4096];
            let _ = s.read(&mut buf).await;
            let body = "data: {\"choices\":[{\"delta\":{\"content\":\"一半\"}}]}\n\n";
            let head = format!("HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ntransfer-encoding: chunked\r\n\r\n{:x}\r\n{body}\r\n", body.len());
            s.write_all(head.as_bytes()).await.unwrap();
            tokio::time::sleep(Duration::from_secs(5)).await;
        });
        let base = format!("http://{addr}/v1");
        let mut req = ChatRequest::new("m", vec![Message::user_text("x")]);
        req.idle_timeout = Duration::from_millis(300);
        let mut got = String::new();
        let r = stream_chat(&Endpoint { kind: ProviderKind::Openai, base_url: &base, key: Some("k") }, &req, |d| got.push_str(d)).await;
        assert_eq!(r, Err(ProviderError::Timeout));
        assert_eq!(got, "一半", "超时前收到的文字已经交出去了");
    }
}
