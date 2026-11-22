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

/// 获取模型列表，返回模型 id（按服务商原顺序，去重）。
///
/// - OpenAI 兼容：`GET {base}/models`，`Authorization: Bearer`，取 `data[].id`
/// - Claude：`GET {base}/v1/models`，`x-api-key` + `anthropic-version`，取 `data[].id`
/// - Gemini：`GET {base}/v1beta/models`，`x-goog-api-key`，取支持 generateContent 的 `models[].name`（去掉 `models/` 前缀）
pub async fn list_models(kind: ProviderKind, base_url: &str, key: Option<&str>) -> Result<Vec<String>, ProviderError> {
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
    let mut ids: Vec<String> = match m.format {
        ApiFormat::OpenAi | ApiFormat::Claude => v
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| ProviderError::Parse("缺少 data 列表".into()))?
            .iter()
            .filter_map(|x| x.get("id").and_then(Value::as_str).map(str::to_string))
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
            .filter_map(|x| x.get("name").and_then(Value::as_str))
            .map(|n| n.trim_start_matches("models/").to_string())
            .collect(),
    };
    let mut seen = std::collections::HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    Ok(ids)
}

/// 按模型名推断标签（用户可以在设置里改）。规则宁可保守：
/// 推理模型、明确的视觉模型、主流多模态系列打「视觉」，其余打「文本」；Ollama 加「本地」。
pub fn infer_tags(kind: ProviderKind, model: &str) -> Vec<ModelTag> {
    let m = model.to_lowercase();
    let has = |words: &[&str]| words.iter().any(|w| m.contains(w));
    let mut tags = Vec::new();
    let vision = has(&["vision", "-vl", "vl-", "vl:", "llava", "pixtral", "gpt-4o", "gpt-4.1", "gpt-5", "omni", "minicpm-v"])
        || (kind == ProviderKind::Anthropic && m.starts_with("claude"))
        || (kind == ProviderKind::Gemini && m.starts_with("gemini"))
        || (m.contains("claude-") && !m.contains("instant"))
        || (m.contains("gemini") && !m.contains("embedding"));
    tags.push(if vision { ModelTag::Vision } else { ModelTag::Text });
    if has(&["reason", "thinking", "-r1", "r1-", "/r1", "o1", "o3", "o4", "qwq"]) {
        tags.push(ModelTag::Reasoning);
    }
    if kind == ProviderKind::Ollama {
        tags.push(ModelTag::Local);
    }
    tags
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
        assert_eq!(infer_tags(ProviderKind::Anthropic, "claude-sonnet-5-5"), vec![Vision]);
        assert_eq!(infer_tags(ProviderKind::Gemini, "gemini-2.5-flash"), vec![Vision]);
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
        let ids = list_models(ProviderKind::Deepseek, &format!("{}/v1/", server.uri()), Some(" sk-test ")).await.unwrap();
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
        assert_eq!(list_models(ProviderKind::Anthropic, &server.uri(), Some("ak")).await.unwrap(), vec!["claude-haiku-5-5"]);
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
        assert_eq!(list_models(ProviderKind::Gemini, &server.uri(), Some("gk")).await.unwrap(), vec!["gemini-2.5-flash"]);
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
        assert_eq!(list_models(ProviderKind::Ollama, &format!("{}/v1", server.uri()), None).await.unwrap(), vec!["qwen2.5:7b"]);
    }
}
