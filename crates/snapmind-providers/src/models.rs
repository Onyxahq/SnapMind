//! 获取模型列表（也用作「测试连接」）与模型标签推断。

use std::time::Duration;

use serde_json::Value;
use snapmind_core::config::{ModelTag, ProviderKind};

use crate::catalog::{meta, ApiFormat};
use crate::error::{check, net, ProviderError};

/// 获取模型列表的超时：测试连接时用户在等，不能太长。
const TIMEOUT: Duration = Duration::from_secs(15);

fn join(base: &str, path: &str) -> String {
    format!("{}/{}", base.trim_end_matches('/'), path.trim_start_matches('/'))
}

/// 列表里的一个模型。`vision` 是服务商自己在列表里说的「能不能看图」，没说就是 None。
#[derive(Debug, Clone, PartialEq)]
pub struct ListedModel {
    pub id: String,
    pub vision: Option<bool>,
    /// 服务商说这个模型支持推理（思考）
    pub reasoning: bool,
}

/// 获取模型列表（按服务商原顺序，去重）。
///
/// - OpenAI 兼容：`GET {base}/models`，`Authorization: Bearer`，取 `data[].id`
/// - Claude：`GET {base}/v1/models`，`x-api-key` + `anthropic-version`，取 `data[].id`
/// - Gemini：`GET {base}/v1beta/models`，`x-goog-api-key`，取支持 generateContent 的 `models[].name`（去掉 `models/` 前缀）
///
/// 列表项里带了输入模态的（OpenRouter、xAI、Mistral、Kimi 等），按它判断能不能看图；
/// Ollama 的 OpenAI 兼容列表不带，再逐个问 `/api/show` 的 capabilities。
pub async fn list_models(kind: ProviderKind, base_url: &str, key: Option<&str>) -> Result<Vec<ListedModel>, ProviderError> {
    let m = meta(kind);
    let key = key.map(str::trim).filter(|k| !k.is_empty());
    if m.needs_key && key.is_none() {
        return Err(ProviderError::MissingKey);
    }
    let client = reqwest::Client::builder().timeout(TIMEOUT).build().map_err(net)?;
    let req = match m.format {
        ApiFormat::OpenAi => {
            let r = client.get(join(base_url, "models"));
            match key {
                Some(k) => r.bearer_auth(k),
                None => r,
            }
        }
        ApiFormat::Claude => client
            .get(join(base_url, "v1/models?limit=1000"))
            .header("x-api-key", key.unwrap_or(""))
            .header("anthropic-version", "2023-06-01"),
        ApiFormat::Gemini => client
            .get(join(base_url, "v1beta/models?pageSize=1000"))
            .header("x-goog-api-key", key.unwrap_or("")),
    };
    let resp = check(req.send().await.map_err(net)?).await?;
    let body = resp.text().await.map_err(net)?;
    let v: Value = serde_json::from_str(&body).map_err(|e| ProviderError::Parse(e.to_string()))?;
    let items: Vec<(String, &Value)> = match m.format {
        ApiFormat::OpenAi | ApiFormat::Claude => v
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Parse("缺少 data 列表".into()))?
            .iter()
            .filter_map(|x| x.get("id").and_then(Value::as_str).map(|id| (id.to_string(), x)))
            .collect(),
        ApiFormat::Gemini => v
            .get("models")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Parse("缺少 models 列表".into()))?
            .iter()
            .filter(|x| {
                x.get("supportedGenerationMethods")
                    .and_then(Value::as_array)
                    .is_none_or(|ms| ms.iter().any(|m| m.as_str() == Some("generateContent")))
            })
            .filter_map(|x| x.get("name").and_then(Value::as_str).map(|n| (n.trim_start_matches("models/").to_string(), x)))
            .collect(),
    };
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<ListedModel> = items
        .into_iter()
        .filter(|(id, _)| seen.insert(id.clone()))
        .map(|(id, x)| ListedModel { vision: vision_from_meta(x), reasoning: reasoning_from_meta(x), id })
        .collect();
    if kind == ProviderKind::Ollama {
        ollama_capabilities(&client, base_url, &mut out).await;
    }
    Ok(out)
}

/// Ollama 原生接口 `POST {根}/api/show` 返回 `capabilities: ["completion", "vision", "thinking"]`。
/// 问不到（老版本、地址不是 Ollama）就不管，退回按名字推断。
async fn ollama_capabilities(client: &reqwest::Client, base_url: &str, models: &mut [ListedModel]) {
    let root = base_url.trim_end_matches('/').trim_end_matches("/v1");
    for m in models.iter_mut().filter(|m| m.vision.is_none()) {
        let Ok(resp) = client.post(join(root, "api/show")).json(&serde_json::json!({ "model": m.id })).send().await else {
            return;
        };
        let Ok(v) = resp.json::<Value>().await else { return };
        let Some(caps) = v.get("capabilities").and_then(Value::as_array) else { return };
        let has = |c: &str| caps.iter().any(|x| x.as_str() == Some(c));
        m.vision = Some(has("vision"));
        m.reasoning |= has("thinking");
    }
}

/// 列表项里的「能不能看图」。各家写法不同，认得的有：
/// - OpenRouter：`architecture.input_modalities: ["text", "image"]`、`architecture.modality: "text+image->text"`
/// - xAI、一些中转站：`input_modalities`；models.dev 风格：`modalities.input`
/// - Mistral：`capabilities.vision: true`；LiteLLM 风格：`supports_vision`
/// - Kimi：`supports_image_in: true`
/// - 字符串数组的 `capabilities`（含 "vision"）
fn vision_from_meta(x: &Value) -> Option<bool> {
    let has_image = |v: &Value| v.as_array().map(|a| a.iter().any(|m| m.as_str().is_some_and(|s| s.eq_ignore_ascii_case("image"))));
    let arch = x.get("architecture");
    if let Some(b) = arch.and_then(|a| a.get("input_modalities")).and_then(has_image) {
        return Some(b);
    }
    if let Some(s) = arch.and_then(|a| a.get("modality")).and_then(Value::as_str) {
        return Some(s.split("->").next().unwrap_or("").contains("image"));
    }
    for v in [x.get("input_modalities"), x.get("modalities").and_then(|m| m.get("input"))].into_iter().flatten() {
        if let Some(b) = has_image(v) {
            return Some(b);
        }
    }
    match x.get("capabilities") {
        Some(Value::Object(c)) => {
            if let Some(b) = c.get("vision").and_then(Value::as_bool) {
                return Some(b);
            }
        }
        Some(Value::Array(a)) if !a.is_empty() => return Some(a.iter().any(|c| c.as_str() == Some("vision"))),
        _ => {}
    }
    ["supports_image_in", "supports_vision"].iter().find_map(|k| x.get(*k).and_then(Value::as_bool))
}

/// OpenRouter 的 `supported_parameters` 里有 reasoning，或者 `capabilities.reasoning` / `supports_reasoning` 为 true。
fn reasoning_from_meta(x: &Value) -> bool {
    let params = x.get("supported_parameters").and_then(Value::as_array);
    params.is_some_and(|a| a.iter().any(|p| matches!(p.as_str(), Some("reasoning" | "include_reasoning"))))
        || x.get("capabilities").and_then(|c| c.get("reasoning")).and_then(Value::as_bool) == Some(true)
        || x.get("supports_reasoning").and_then(Value::as_bool) == Some(true)
}

/// 打标签：服务商说了能不能看图就听它的，没说再按名字推断（用户可以在设置里改）。
pub fn tags_for(kind: ProviderKind, m: &ListedModel) -> Vec<ModelTag> {
    let mut tags = infer_tags(kind, &m.id);
    if let Some(v) = m.vision {
        tags[0] = if v { ModelTag::Vision } else { ModelTag::Text };
    }
    if m.reasoning && !tags.contains(&ModelTag::Reasoning) {
        tags.insert(1, ModelTag::Reasoning);
    }
    tags
}

/// 按模型名推断标签。第一个一定是「视觉」或「文本」；推理模型加「推理」；本机服务加「本地」。
///
/// 先查随程序打包的 models.dev 目录（[`known`]），查不到再按名字规则猜。
pub fn infer_tags(kind: ProviderKind, model: &str) -> Vec<ModelTag> {
    let m = model.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| m.contains(w));
    let named = vision_by_name(&m);
    let (vision, reasoning) = match known(&m) {
        // 目录里有的托管方只开放文字输入（例如部分 Gemma 3 量化版），所以厂商文档里明确能看图的系列也算
        Some((v, r)) => (v || named, r),
        None => (named, has(&["reason", "thinking", "-r1", "r1-", "/r1", "o1", "o3", "o4", "qwq", "qvq", "magistral"])),
    };
    let mut tags = vec![if vision { ModelTag::Vision } else { ModelTag::Text }];
    if reasoning {
        tags.push(ModelTag::Reasoning);
    }
    if matches!(kind, ProviderKind::Ollama | ProviderKind::Lmstudio) {
        tags.push(ModelTag::Local);
    }
    tags
}

/// models.dev 目录的精简索引（`scripts/vision-index.mjs` 生成，发版前刷新）：模型名 → 标记（v 看图、r 推理）。
const MODELS_DEV: &str = include_str!("../data/models-dev.tsv");

fn models_dev() -> &'static std::collections::HashMap<&'static str, &'static str> {
    static INDEX: std::sync::OnceLock<std::collections::HashMap<&'static str, &'static str>> = std::sync::OnceLock::new();
    INDEX.get_or_init(|| {
        MODELS_DEV
            .lines()
            .filter(|l| !l.starts_with('#'))
            .filter_map(|l| l.split_once('\t'))
            .collect()
    })
}

/// 在 models.dev 索引里查（能不能看图，是不是推理模型）。写法和索引对齐：去掉组织前缀、`~` 和冒号后缀；
/// 查不到再去掉末尾的日期版本（-2025-04-14、-20250514、-250615、-0905）试一次。
fn known(m: &str) -> Option<(bool, bool)> {
    let bare = m.rsplit('/').next().unwrap_or(m).trim_start_matches('~');
    let bare = bare.split(':').next().unwrap_or(bare);
    let idx = models_dev();
    let hit = idx.get(bare).or_else(|| idx.get(strip_date(bare)?))?;
    Some((hit.contains('v'), hit.contains('r')))
}

fn strip_date(id: &str) -> Option<&str> {
    let is_date = |s: &str| (4..=8).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_digit());
    if let Some((head, tail)) = id.rsplit_once('-') {
        if is_date(tail) {
            return Some(head);
        }
    }
    // -2025-04-14
    let b = id.as_bytes();
    let n = b.len();
    if n > 11 && b[n - 11] == b'-' && b[n - 6] == b'-' && b[n - 3] == b'-' {
        let date = &id[n - 10..];
        if date.bytes().filter(u8::is_ascii_digit).count() == 8 {
            return Some(&id[..n - 11]);
        }
    }
    None
}

/// 名字里带这些的不是拿来看图聊天的模型（嵌入、重排、语音、审核、生图……）。
const NOT_CHAT: &[&str] = &[
    "embed", "rerank", "tts", "whisper", "transcribe", "moderation", "-audio", "audio-", "speech", "imagen", "dall-e", "veo-",
];

/// 能看图系列里的纯文字型号（小尺寸、mini 推理版、翻译专用）。
const TEXT_ONLY: &[&str] = &[
    "o1-mini", "o3-mini", "gemma-3-1b", "gemma3:1b", "gemma-3-270m", "gemma3:270m", "nova-micro", "doubao-seed-translation", "gpt-oss",
    "claude-2", "claude-instant", "claude-1",
];

/// 各家官方文档里标明支持图片输入的系列（小写子串匹配，带组织前缀的写法如 `qwen/qwen3-vl` 也能命中）。
const VISION_NAMES: &[&str] = &[
    // 通用叫法
    "vision", "-vl", "vl-", "vl:", "_vl", "vlm", "omni", "multimodal", "llava", "ocr",
    // Anthropic：Claude 3 起全系列；Google：Gemini 全系列
    "claude", "gemini",
    // OpenAI：GPT-4o / 4.1 / 4.5 / 5、o4-mini、computer-use（o1 / o3 见 vision_by_name）
    "gpt-4o", "gpt-4.1", "gpt-4.5", "gpt-4-turbo", "gpt-5", "chatgpt-4o", "o4-mini", "computer-use",
    // Mistral：Pixtral、Small 3.1 起、Medium 3、Large 3、Magistral 2509 起、Ministral 3
    "pixtral", "mistral-small-3", "mistral-small-25", "mistral-small-latest", "mistral-medium", "mistral-large-3", "mistral-large-25",
    "magistral-small-25", "magistral-medium-25", "ministral-3",
    // Meta：Llama 4（Scout / Maverick 原生多模态）
    "llama-4", "llama4",
    // Google 开放模型：Gemma 3（1B / 270M 除外）、Gemma 3n、PaliGemma
    "gemma-3", "gemma3", "paligemma",
    // xAI：Grok 4 系列
    "grok-4",
    // 通义：QVQ、Qwen3.5 / 3.6（原生多模态）
    "qvq", "qwen3.5", "qwen3.6",
    // Kimi：K2.5（原生多模态）、kimi-latest
    "kimi-k2.5", "kimi-k2-5", "kimi-latest",
    // 豆包：Seed 1.6 起都能看图（翻译模型在 TEXT_ONLY 里排除）
    "doubao-seed-1", "doubao-seed-2",
    // 阶跃星辰：Step-1V / 1o / 1.5V / R1-V、Step 3
    "step-1v", "step-1o", "step-1.5v", "step-r1-v", "step-3",
    // 百度：ERNIE 5（原生全模态）
    "ernie-5",
    // DeepSeek：VL2、Janus
    "deepseek-vl", "janus",
    // 开源视觉模型
    "minicpm-v", "minicpm-o", "internvl", "cogvlm", "moondream", "molmo", "xcomposer",
    // Amazon Nova（Micro 只有文字）
    "nova-lite", "nova-pro", "nova-premier", "nova-2-lite",
];

fn vision_by_name(m: &str) -> bool {
    if NOT_CHAT.iter().chain(TEXT_ONLY).any(|w| m.contains(w)) {
        return false;
    }
    if VISION_NAMES.iter().any(|w| m.contains(w)) || glm_vision(m) {
        return true;
    }
    // o1 / o3 正式版能看图（mini 版在 TEXT_ONLY 里排除了）
    let bare = m.rsplit('/').next().unwrap_or(m);
    ["o1", "o3"].iter().any(|o| bare == *o || bare.starts_with(&format!("{o}-2")) || bare.starts_with(&format!("{o}-pro")))
}

/// 智谱的视觉模型是版本号后面紧跟 v：glm-4v、glm-4.1v-thinking、glm-4.5v、glm-4.6v。
fn glm_vision(m: &str) -> bool {
    m.match_indices("glm-").any(|(i, _)| {
        let rest = &m[i + 4..];
        let ver = rest.find(|c: char| !(c.is_ascii_digit() || c == '.')).unwrap_or(rest.len());
        ver > 0 && rest[ver..].starts_with('v')
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[test]
    fn tags() {
        use ModelTag::*;
        assert_eq!(infer_tags(ProviderKind::Deepseek, "deepseek-chat"), vec![Text]);
        assert_eq!(infer_tags(ProviderKind::Deepseek, "deepseek-reasoner"), vec![Text, Reasoning]);
        // 这两个的「推理」来自 models.dev 目录
        assert_eq!(infer_tags(ProviderKind::Anthropic, "claude-sonnet-5-5"), vec![Vision, Reasoning]);
        assert_eq!(infer_tags(ProviderKind::Gemini, "gemini-2.5-flash"), vec![Vision, Reasoning]);
        // 目录里没有、按名字规则
        assert_eq!(infer_tags(ProviderKind::Custom, "brand-new-vl-9b"), vec![Vision]);
        assert_eq!(infer_tags(ProviderKind::Custom, "brand-new-chat-9b"), vec![Text]);
        // 带日期后缀的型号去掉日期再查
        assert_eq!(strip_date("gpt-4.1-2025-04-14"), Some("gpt-4.1"));
        assert_eq!(strip_date("doubao-seed-1-6-250615"), Some("doubao-seed-1-6"));
        assert_eq!(strip_date("kimi-k3"), None);
        assert_eq!(infer_tags(ProviderKind::Moonshot, "kimi-k3")[0], Vision);
        assert_eq!(infer_tags(ProviderKind::Openrouter, "qwen/qwen2.5-vl-72b-instruct"), vec![Vision]);
        assert_eq!(infer_tags(ProviderKind::Ollama, "qwen2.5vl:7b"), vec![Vision, Local]);
        assert_eq!(infer_tags(ProviderKind::Ollama, "qwen2.5:7b"), vec![Text, Local]);
        assert_eq!(infer_tags(ProviderKind::Openai, "o3-mini"), vec![Text, Reasoning]);
    }

    #[tokio::test]
    async fn openai_compatible_lists_ids_with_bearer() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("authorization", "Bearer sk-test"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "data": [{ "id": "deepseek-chat" }, { "id": "deepseek-reasoner" }, { "id": "deepseek-chat" }]
            })))
            .mount(&server)
            .await;
        let ids = list_models(ProviderKind::Deepseek, &format!("{}/v1/", server.uri()), Some(" sk-test ")).await.unwrap().into_iter().map(|m| m.id).collect::<Vec<_>>();
        assert_eq!(ids, vec!["deepseek-chat", "deepseek-reasoner"]);
    }

    #[tokio::test]
    async fn claude_uses_x_api_key_and_version() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("x-api-key", "ak"))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [{ "id": "claude-haiku-5-5" }] })))
            .mount(&server)
            .await;
        assert_eq!(list_models(ProviderKind::Anthropic, &server.uri(), Some("ak")).await.unwrap().into_iter().map(|m| m.id).collect::<Vec<_>>(), vec!["claude-haiku-5-5"]);
    }

    #[tokio::test]
    async fn gemini_filters_generate_content_and_strips_prefix() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1beta/models"))
            .and(header("x-goog-api-key", "gk"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "models": [
                { "name": "models/gemini-2.5-flash", "supportedGenerationMethods": ["generateContent", "countTokens"] },
                { "name": "models/text-embedding-004", "supportedGenerationMethods": ["embedContent"] }
            ]})))
            .mount(&server)
            .await;
        assert_eq!(list_models(ProviderKind::Gemini, &server.uri(), Some("gk")).await.unwrap().into_iter().map(|m| m.id).collect::<Vec<_>>(), vec!["gemini-2.5-flash"]);
    }

    #[tokio::test]
    async fn errors_are_classified() {
        let server = MockServer::start().await;
        Mock::given(path("/bad/models")).respond_with(ResponseTemplate::new(401)).mount(&server).await;
        Mock::given(path("/err/models"))
            .respond_with(ResponseTemplate::new(429).set_body_json(serde_json::json!({ "error": { "message": "rate limited" } })))
            .mount(&server)
            .await;
        Mock::given(path("/junk/models")).respond_with(ResponseTemplate::new(200).set_body_string("<html>")).mount(&server).await;
        let k = Some("k");
        assert_eq!(list_models(ProviderKind::Openai, &format!("{}/bad", server.uri()), k).await, Err(ProviderError::Auth(401)));
        assert_eq!(
            list_models(ProviderKind::Openai, &format!("{}/err", server.uri()), k).await,
            Err(ProviderError::Http { status: 429, message: "rate limited".into() })
        );
        assert!(matches!(list_models(ProviderKind::Openai, &format!("{}/junk", server.uri()), k).await, Err(ProviderError::Parse(_))));
        assert_eq!(list_models(ProviderKind::Openai, "http://x", None).await, Err(ProviderError::MissingKey));
        // 连接不上
        assert!(matches!(list_models(ProviderKind::Openai, "http://127.0.0.1:1", k).await, Err(ProviderError::Network(_))));
    }

    #[tokio::test]
    async fn ollama_needs_no_key() {
        let server = MockServer::start().await;
        Mock::given(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [{ "id": "qwen2.5:7b" }] })))
            .mount(&server)
            .await;
        assert_eq!(list_models(ProviderKind::Ollama, &format!("{}/v1", server.uri()), None).await.unwrap().into_iter().map(|m| m.id).collect::<Vec<_>>(), vec!["qwen2.5:7b"]);
    }

    #[test]
    fn vision_names_follow_vendor_docs() {
        let vision = |id: &str| infer_tags(ProviderKind::Openrouter, id)[0] == ModelTag::Vision;
        for id in [
            "openai/gpt-4o-mini", "openai/gpt-4.1", "openai/gpt-5-mini", "openai/o3", "openai/o1", "o4-mini",
            "anthropic/claude-3.5-haiku", "claude-opus-4-1", "google/gemini-2.5-pro",
            "x-ai/grok-4", "x-ai/grok-2-vision-1212", "meta-llama/llama-4-maverick", "llama3.2-vision:11b",
            "google/gemma-3-27b-it", "gemma3:4b", "mistralai/mistral-small-3.1-24b-instruct", "pixtral-large-latest",
            "mistral-medium-2508", "z-ai/glm-4.5v", "thudm/glm-4.1v-9b-thinking", "glm-4v-plus",
            "moonshotai/kimi-k2.5", "moonshot-v1-8k-vision-preview", "qwen/qwen3-vl-235b-a22b-instruct",
            "qwen-vl-max", "qwen2.5vl:7b", "qwen/qvq-72b-preview", "qwen3.5-plus", "qwen-omni-turbo",
            "doubao-seed-1-6-250615", "doubao-1-5-vision-pro-32k", "step-1v-8k", "step-3",
            "deepseek-ai/deepseek-vl2", "deepseek-ai/deepseek-ocr", "paddlepaddle/paddleocr-vl",
            "amazon/nova-lite-v1", "internvl3-78b", "minicpm-v:8b", "ernie-4.5-turbo-vl-32k", "hunyuan-vision",
            "minimax/minimax-vl-01", "microsoft/phi-4-multimodal-instruct",
        ] {
            assert!(vision(id), "{id} 应该能看图");
        }
        for id in [
            "deepseek-chat", "deepseek-reasoner", "openai/o3-mini", "o1-mini", "openai/gpt-oss-120b", "gpt-4-0613",
            "google/gemma-3-1b-it", "text-embedding-3-small", "gemini-embedding-001", "gpt-4o-mini-tts",
            "omni-moderation-latest", "whisper-1", "kimi-k2-0905-preview", "glm-4.6", "glm-4.5-air", "x-ai/grok-3",
            "grok-code-fast-1", "qwen3-235b-a22b", "qwen-max", "mistral-large-2411", "devstral-medium-2507",
            "amazon/nova-micro-v1", "claude-2.1", "doubao-seed-translation-250915", "MiniMax-M2", "ernie-4.5-turbo-128k",
        ] {
            assert!(!vision(id), "{id} 应该是纯文本");
        }
    }

    #[test]
    fn vision_from_listing_metadata() {
        use serde_json::json;
        // OpenRouter
        assert_eq!(vision_from_meta(&json!({ "architecture": { "input_modalities": ["text", "image"], "modality": "text+image->text" } })), Some(true));
        assert_eq!(vision_from_meta(&json!({ "architecture": { "input_modalities": ["text"] } })), Some(false));
        assert_eq!(vision_from_meta(&json!({ "architecture": { "modality": "text+image->text" } })), Some(true));
        assert_eq!(vision_from_meta(&json!({ "architecture": { "modality": "text->image" } })), Some(false));
        // xAI / models.dev
        assert_eq!(vision_from_meta(&json!({ "input_modalities": ["text", "image"] })), Some(true));
        assert_eq!(vision_from_meta(&json!({ "modalities": { "input": ["text"], "output": ["text"] } })), Some(false));
        // Mistral、Kimi、字符串数组
        assert_eq!(vision_from_meta(&json!({ "capabilities": { "completion_chat": true, "vision": true } })), Some(true));
        assert_eq!(vision_from_meta(&json!({ "supports_image_in": false })), Some(false));
        assert_eq!(vision_from_meta(&json!({ "capabilities": ["completion", "vision"] })), Some(true));
        // 什么都没说
        assert_eq!(vision_from_meta(&json!({ "id": "x", "object": "model" })), None);
        assert!(reasoning_from_meta(&json!({ "supported_parameters": ["tools", "reasoning"] })));
        assert!(!reasoning_from_meta(&json!({ "supported_parameters": ["tools"] })));
    }

    #[test]
    fn metadata_overrides_name_guess() {
        use ModelTag::*;
        // 名字看不出来、OpenRouter 说能看图
        let m = ListedModel { id: "some-lab/new-model".into(), vision: Some(true), reasoning: true };
        assert_eq!(tags_for(ProviderKind::Openrouter, &m), vec![Vision, Reasoning]);
        // 名字像视觉模型、服务商说不能
        let m = ListedModel { id: "acme/omni-text".into(), vision: Some(false), reasoning: false };
        assert_eq!(tags_for(ProviderKind::Openrouter, &m), vec![Text]);
        let m = ListedModel { id: "qwen2.5vl:7b".into(), vision: None, reasoning: false };
        assert_eq!(tags_for(ProviderKind::Ollama, &m), vec![Vision, Local]);
        assert_eq!(infer_tags(ProviderKind::Lmstudio, "gemma-3-12b"), vec![Vision, Local]);
    }

    #[tokio::test]
    async fn openrouter_listing_carries_modalities() {
        let server = MockServer::start().await;
        Mock::given(path("/api/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [
                { "id": "new-lab/seer-1", "architecture": { "input_modalities": ["text", "image"] } },
                { "id": "new-lab/scribe-1", "architecture": { "input_modalities": ["text"] }, "supported_parameters": ["reasoning"] }
            ]})))
            .mount(&server)
            .await;
        let got = list_models(ProviderKind::Openrouter, &format!("{}/api/v1", server.uri()), Some("k")).await.unwrap();
        assert_eq!(got[0], ListedModel { id: "new-lab/seer-1".into(), vision: Some(true), reasoning: false });
        assert_eq!(got[1], ListedModel { id: "new-lab/scribe-1".into(), vision: Some(false), reasoning: true });
    }

    #[tokio::test]
    async fn ollama_asks_api_show_for_capabilities() {
        use wiremock::matchers::body_partial_json;
        let server = MockServer::start().await;
        Mock::given(path("/v1/models"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "data": [{ "id": "mystery:7b" }, { "id": "plain:3b" }] })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/show"))
            .and(body_partial_json(serde_json::json!({ "model": "mystery:7b" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "capabilities": ["completion", "vision", "thinking"] })))
            .mount(&server)
            .await;
        Mock::given(method("POST"))
            .and(path("/api/show"))
            .and(body_partial_json(serde_json::json!({ "model": "plain:3b" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({ "capabilities": ["completion"] })))
            .mount(&server)
            .await;
        let got = list_models(ProviderKind::Ollama, &format!("{}/v1", server.uri()), None).await.unwrap();
        assert_eq!(got[0], ListedModel { id: "mystery:7b".into(), vision: Some(true), reasoning: true });
        assert_eq!(got[1], ListedModel { id: "plain:3b".into(), vision: Some(false), reasoning: false });
    }
}
