//! 机器翻译：DeepL、Google Cloud Translation（v2）、微软 Translator（v3），都用官方接口。
//!
//! 语言代码统一用 BCP 47 风格（`zh-Hans`、`zh-Hant`、`en`、`ja`……），在这里换成各家的写法；
//! 检测到的原文语言也换回统一写法，供「原文已是目标语言」的判断使用。
//!
//! 文本按行拆成数组发送、按原样拼回：三家都能保证一一对应，空行和行数不会被改动。

use std::time::Duration;

use serde_json::{json, Value};
use snapmind_core::config::MtKind;

use crate::error::{check, net, ProviderError};

/// 要调用的机器翻译服务。`base_url` 只在测试时覆盖默认地址。
#[derive(Debug, Clone, Copy)]
pub struct MtEndpoint<'a> {
    pub kind: MtKind,
    pub key: &'a str,
    /// 微软翻译的区域（全局资源为 None）
    pub region: Option<&'a str>,
    pub base_url: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MtOutput {
    pub text: String,
    /// 检测到的原文语言（统一写法），服务没给时为 None
    pub detected: Option<String>,
}

/// 每次请求最多几段（DeepL 上限 50，Google 128，微软 1000，取最小）。
const BATCH: usize = 50;

fn default_base(kind: MtKind, key: &str) -> &'static str {
    match kind {
        // DeepL 免费版的 Key 以 :fx 结尾，走单独的域名
        MtKind::Deepl if key.trim().ends_with(":fx") => "https://api-free.deepl.com",
        MtKind::Deepl => "https://api.deepl.com",
        MtKind::Google => "https://translation.googleapis.com",
        MtKind::Microsoft => "https://api.cognitive.microsofttranslator.com",
    }
}

/// 统一写法 → 各家写法。
pub fn to_service_lang(kind: MtKind, lang: &str, is_target: bool) -> String {
    let l = lang.trim();
    let lower = l.to_ascii_lowercase();
    match kind {
        MtKind::Deepl => {
            if !is_target {
                // DeepL 的原文语言不分变体
                return lower.split('-').next().unwrap_or("").to_ascii_uppercase();
            }
            match lower.as_str() {
                "zh" | "zh-hans" | "zh-cn" => "ZH-HANS".into(),
                "zh-hant" | "zh-tw" | "zh-hk" => "ZH-HANT".into(),
                "en" => "EN-US".into(),
                "pt" => "PT-BR".into(),
                _ => l.to_ascii_uppercase(),
            }
        }
        MtKind::Google => match lower.as_str() {
            "zh" | "zh-hans" => "zh-CN".into(),
            "zh-hant" => "zh-TW".into(),
            _ => l.to_string(),
        },
        MtKind::Microsoft => match lower.as_str() {
            "zh" | "zh-cn" => "zh-Hans".into(),
            "zh-tw" | "zh-hk" => "zh-Hant".into(),
            _ => l.to_string(),
        },
    }
}

/// 各家写法 → 统一写法（只处理中文的几种写法和大小写，其他保持）。
pub fn from_service_lang(lang: &str) -> String {
    match lang.trim().to_ascii_lowercase().as_str() {
        "zh" | "zh-cn" | "zh-hans" | "zh-sg" => "zh-Hans".into(),
        "zh-tw" | "zh-hk" | "zh-hant" => "zh-Hant".into(),
        other => other.split('-').next().unwrap_or(other).to_string(),
    }
}

/// 翻译一段文本（可含多行）。`source` 为 None 表示自动检测。
pub async fn translate(
    ep: &MtEndpoint<'_>,
    text: &str,
    source: Option<&str>,
    target: &str,
    timeout: Duration,
) -> Result<MtOutput, ProviderError> {
    let key = ep.key.trim();
    if key.is_empty() {
        return Err(ProviderError::MissingKey);
    }
    // 只翻非空行，空行原样留位
    let lines: Vec<&str> = text.split('\n').collect();
    let todo: Vec<(usize, &str)> = lines
        .iter()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| (i, l.trim_end_matches('\r')))
        .collect();
    if todo.is_empty() {
        return Err(ProviderError::Empty);
    }
    let client = reqwest::Client::builder().timeout(timeout).build().map_err(net)?;
    let base = ep.base_url.unwrap_or_else(|| default_base(ep.kind, key)).trim_end_matches('/');
    let mut out: Vec<String> = lines.iter().map(|_| String::new()).collect();
    let mut detected = None;
    for batch in todo.chunks(BATCH) {
        let texts: Vec<&str> = batch.iter().map(|(_, t)| *t).collect();
        let (got, det) = request(&client, ep, key, base, &texts, source, target).await?;
        if got.len() != texts.len() {
            return Err(ProviderError::Parse(format!("发出 {} 段，收到 {} 段", texts.len(), got.len())));
        }
        if detected.is_none() {
            detected = det;
        }
        for ((i, _), t) in batch.iter().zip(got) {
            out[*i] = t;
        }
    }
    let text = out.join("\n");
    if text.trim().is_empty() {
        return Err(ProviderError::Empty);
    }
    Ok(MtOutput { text, detected })
}

async fn request(
    client: &reqwest::Client,
    ep: &MtEndpoint<'_>,
    key: &str,
    base: &str,
    texts: &[&str],
    source: Option<&str>,
    target: &str,
) -> Result<(Vec<String>, Option<String>), ProviderError> {
    let source = source.map(str::trim).filter(|s| !s.is_empty() && *s != "auto");
    let req = match ep.kind {
        MtKind::Deepl => {
            let mut body = json!({ "text": texts, "target_lang": to_service_lang(MtKind::Deepl, target, true) });
            if let Some(s) = source {
                body["source_lang"] = json!(to_service_lang(MtKind::Deepl, s, false));
            }
            client.post(format!("{base}/v2/translate")).header("authorization", format!("DeepL-Auth-Key {key}")).json(&body)
        }
        MtKind::Google => {
            let mut body = json!({ "q": texts, "target": to_service_lang(MtKind::Google, target, true), "format": "text" });
            if let Some(s) = source {
                body["source"] = json!(to_service_lang(MtKind::Google, s, false));
            }
            client.post(format!("{base}/language/translate/v2")).header("x-goog-api-key", key).json(&body)
        }
        MtKind::Microsoft => {
            let mut query = vec![("api-version", "3.0".to_string()), ("to", to_service_lang(MtKind::Microsoft, target, true))];
            if let Some(s) = source {
                query.push(("from", to_service_lang(MtKind::Microsoft, s, false)));
            }
            let body: Vec<Value> = texts.iter().map(|t| json!({ "Text": t })).collect();
            let mut r = client
                .post(format!("{base}/translate"))
                .query(&query)
                .header("Ocp-Apim-Subscription-Key", key)
                .json(&body);
            if let Some(region) = ep.region.map(str::trim).filter(|r| !r.is_empty()) {
                r = r.header("Ocp-Apim-Subscription-Region", region);
            }
            r
        }
    };
    let resp = check(req.send().await.map_err(net)?).await?;
    let v: Value = resp.json().await.map_err(|e| ProviderError::Parse(e.to_string()))?;
    let bad = || ProviderError::Parse("缺少译文".into());
    let str_at = |x: &Value, p: &str| x.pointer(p).and_then(Value::as_str).map(str::to_string);
    match ep.kind {
        MtKind::Deepl => {
            let arr = v.get("translations").and_then(Value::as_array).ok_or_else(bad)?;
            let det = arr.first().and_then(|x| str_at(x, "/detected_source_language")).map(|l| from_service_lang(&l));
            Ok((arr.iter().map(|x| str_at(x, "/text").unwrap_or_default()).collect(), det))
        }
        MtKind::Google => {
            let arr = v.pointer("/data/translations").and_then(Value::as_array).ok_or_else(bad)?;
            let det = arr.first().and_then(|x| str_at(x, "/detectedSourceLanguage")).map(|l| from_service_lang(&l));
            Ok((arr.iter().map(|x| str_at(x, "/translatedText").unwrap_or_default()).collect(), det))
        }
        MtKind::Microsoft => {
            let arr = v.as_array().ok_or_else(bad)?;
            let det = arr.first().and_then(|x| str_at(x, "/detectedLanguage/language")).map(|l| from_service_lang(&l));
            Ok((arr.iter().map(|x| str_at(x, "/translations/0/text").unwrap_or_default()).collect(), det))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{body_json, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const T: Duration = Duration::from_secs(5);

    #[test]
    fn language_codes() {
        assert_eq!(to_service_lang(MtKind::Deepl, "zh-Hans", true), "ZH-HANS");
        assert_eq!(to_service_lang(MtKind::Deepl, "en", true), "EN-US");
        assert_eq!(to_service_lang(MtKind::Deepl, "zh-Hant", false), "ZH");
        assert_eq!(to_service_lang(MtKind::Deepl, "ja", true), "JA");
        assert_eq!(to_service_lang(MtKind::Google, "zh-Hans", true), "zh-CN");
        assert_eq!(to_service_lang(MtKind::Google, "zh-Hant", true), "zh-TW");
        assert_eq!(to_service_lang(MtKind::Microsoft, "zh-Hans", true), "zh-Hans");
        assert_eq!(from_service_lang("ZH"), "zh-Hans");
        assert_eq!(from_service_lang("zh-TW"), "zh-Hant");
        assert_eq!(from_service_lang("EN"), "en");
        assert_eq!(from_service_lang("en-GB"), "en");
        assert_eq!(default_base(MtKind::Deepl, "abc:fx"), "https://api-free.deepl.com");
        assert_eq!(default_base(MtKind::Deepl, "abc"), "https://api.deepl.com");
    }

    #[tokio::test]
    async fn deepl_keeps_empty_lines_and_detects_source() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/translate"))
            .and(header("authorization", "DeepL-Auth-Key dk"))
            .and(body_json(json!({ "text": ["Hello", "World"], "target_lang": "ZH-HANS" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "translations": [
                { "detected_source_language": "EN", "text": "你好" },
                { "detected_source_language": "EN", "text": "世界" }
            ]})))
            .mount(&server)
            .await;
        let base = server.uri();
        let ep = MtEndpoint { kind: MtKind::Deepl, key: "dk", region: None, base_url: Some(&base) };
        let r = translate(&ep, "Hello\n\nWorld", None, "zh-Hans", T).await.unwrap();
        assert_eq!(r, MtOutput { text: "你好\n\n世界".into(), detected: Some("en".into()) });
    }

    #[tokio::test]
    async fn google_v2_with_source_language() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/language/translate/v2"))
            .and(header("x-goog-api-key", "gk"))
            .and(body_json(json!({ "q": ["こんにちは"], "target": "zh-CN", "format": "text", "source": "ja" })))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "data": { "translations": [{ "translatedText": "你好" }] } })))
            .mount(&server)
            .await;
        let base = server.uri();
        let ep = MtEndpoint { kind: MtKind::Google, key: "gk", region: None, base_url: Some(&base) };
        let r = translate(&ep, "こんにちは", Some("ja"), "zh-Hans", T).await.unwrap();
        assert_eq!(r, MtOutput { text: "你好".into(), detected: None });
    }

    #[tokio::test]
    async fn microsoft_v3_with_region_header() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/translate"))
            .and(query_param("api-version", "3.0"))
            .and(query_param("to", "zh-Hant"))
            .and(header("Ocp-Apim-Subscription-Key", "mk"))
            .and(header("Ocp-Apim-Subscription-Region", "eastasia"))
            .and(body_json(json!([{ "Text": "Good" }])))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "detectedLanguage": { "language": "en", "score": 1.0 }, "translations": [{ "text": "好", "to": "zh-Hant" }] }
            ])))
            .mount(&server)
            .await;
        let base = server.uri();
        let ep = MtEndpoint { kind: MtKind::Microsoft, key: "mk", region: Some("eastasia"), base_url: Some(&base) };
        let r = translate(&ep, "Good", Some("auto"), "zh-Hant", T).await.unwrap();
        assert_eq!(r, MtOutput { text: "好".into(), detected: Some("en".into()) });
    }

    #[tokio::test]
    async fn long_text_is_batched() {
        let server = MockServer::start().await;
        // 回显：把每段原文加上前缀
        struct Echo;
        impl wiremock::Respond for Echo {
            fn respond(&self, req: &wiremock::Request) -> ResponseTemplate {
                let v: Value = serde_json::from_slice(&req.body).unwrap();
                let tr: Vec<Value> = v["text"].as_array().unwrap().iter().map(|t| json!({ "text": format!("译{}", t.as_str().unwrap()) })).collect();
                ResponseTemplate::new(200).set_body_json(json!({ "translations": tr }))
            }
        }
        Mock::given(path("/v2/translate")).respond_with(Echo).expect(3).mount(&server).await;
        let base = server.uri();
        let ep = MtEndpoint { kind: MtKind::Deepl, key: "k", region: None, base_url: Some(&base) };
        let src: Vec<String> = (0..120).map(|i| i.to_string()).collect();
        let r = translate(&ep, &src.join("\n"), None, "zh-Hans", T).await.unwrap();
        let want: Vec<String> = (0..120).map(|i| format!("译{i}")).collect();
        assert_eq!(r.text, want.join("\n"));
    }

    #[tokio::test]
    async fn errors() {
        let server = MockServer::start().await;
        Mock::given(path("/a/v2/translate")).respond_with(ResponseTemplate::new(403)).mount(&server).await;
        Mock::given(path("/q/v2/translate"))
            .respond_with(ResponseTemplate::new(456).set_body_json(json!({ "message": "Quota exceeded" })))
            .mount(&server)
            .await;
        Mock::given(path("/e/v2/translate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "translations": [{ "text": " " }] })))
            .mount(&server)
            .await;
        Mock::given(path("/n/v2/translate"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "translations": [] })))
            .mount(&server)
            .await;
        Mock::given(path("/s/v2/translate"))
            .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(3)))
            .mount(&server)
            .await;
        let call = |sub: &str, text: &'static str, t: Duration| {
            let base = format!("{}/{sub}", server.uri());
            async move { translate(&MtEndpoint { kind: MtKind::Deepl, key: "k", region: None, base_url: Some(&base) }, text, None, "zh-Hans", t).await }
        };
        assert_eq!(call("a", "x", T).await, Err(ProviderError::Auth(403)));
        assert_eq!(call("q", "x", T).await, Err(ProviderError::Http { status: 456, message: "Quota exceeded".into() }));
        assert_eq!(call("e", "x", T).await, Err(ProviderError::Empty));
        assert!(matches!(call("n", "x", T).await, Err(ProviderError::Parse(_))));
        assert_eq!(call("s", "x", Duration::from_millis(300)).await, Err(ProviderError::Timeout));
        assert_eq!(call("a", " \n ", T).await, Err(ProviderError::Empty));
        let ep = MtEndpoint { kind: MtKind::Google, key: " ", region: None, base_url: None };
        assert_eq!(translate(&ep, "x", None, "en", T).await, Err(ProviderError::MissingKey));
    }
}
