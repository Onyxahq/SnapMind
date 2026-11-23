//! 所有适配器共用的错误类型，消息直接给用户看（也会写进「已切换到……」的说明）。

use serde_json::Value;
use snapmind_core::i18n::{tr, trf};

#[derive(Debug, Clone, PartialEq)]
pub enum ProviderError {
    MissingKey,
    Auth(u16),
    Http { status: u16, message: String },
    /// 流式响应中途返回的错误
    Remote(String),
    Refused(String),
    Network(String),
    Timeout,
    Parse(String),
    Empty,
    /// 后端暂时用不了（如本地 OCR 还没安装）
    Unavailable(String),
}

/// 消息直接给用户看，所以按界面语言翻译（词典在 snapmind-core）。
impl std::fmt::Display for ProviderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            ProviderError::MissingKey => tr("请先填写 API Key"),
            ProviderError::Auth(code) => trf("API Key 无效或没有权限（HTTP {code}）", &[("code", &code.to_string())]),
            ProviderError::Http { status, message } => {
                trf("服务返回错误（HTTP {status}）：{message}", &[("status", &status.to_string()), ("message", message)])
            }
            ProviderError::Remote(m) => trf("服务返回错误：{message}", &[("message", m)]),
            ProviderError::Refused(m) => trf("内容被模型拒绝：{message}", &[("message", m)]),
            ProviderError::Network(m) => trf("连接失败：{message}", &[("message", m)]),
            ProviderError::Timeout => tr("等待响应超时"),
            ProviderError::Parse(m) => trf("返回内容无法解析：{message}", &[("message", m)]),
            ProviderError::Empty => tr("模型返回了空结果"),
            ProviderError::Unavailable(m) => m.clone(),
        };
        f.write_str(&text)
    }
}

impl std::error::Error for ProviderError {}

impl ProviderError {
    /// 写进「deepseek-chat 超时，已切换到 DeepL」里的短说明。
    pub fn short(&self) -> String {
        match self {
            ProviderError::MissingKey => tr("没有 Key"),
            ProviderError::Auth(_) => tr("Key 无效"),
            ProviderError::Http { status, .. } => trf("出错（HTTP {status}）", &[("status", &status.to_string())]),
            ProviderError::Remote(_) => tr("出错"),
            ProviderError::Refused(_) => tr("拒绝回答"),
            ProviderError::Network(_) => tr("连不上"),
            ProviderError::Timeout => tr("超时"),
            ProviderError::Parse(_) => tr("返回格式不对"),
            ProviderError::Empty => tr("返回空结果"),
            ProviderError::Unavailable(_) => tr("不可用"),
        }
    }
}

pub(crate) fn net(e: reqwest::Error) -> ProviderError {
    if e.is_timeout() {
        ProviderError::Timeout
    } else {
        ProviderError::Network(e.to_string())
    }
}

/// 从错误响应里尽量取出可读的错误信息（各家格式不同）。
pub(crate) fn error_message(body: &str) -> String {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    // Gemini 流式接口的错误包在数组里
    let v = v.as_array().and_then(|a| a.first().cloned()).unwrap_or(v);
    let msg = v
        .pointer("/error/message")
        .or_else(|| v.pointer("/error"))
        .or_else(|| v.pointer("/message"))
        .and_then(|m| m.as_str().map(str::to_string));
    msg.unwrap_or_else(|| body.trim().chars().take(200).collect())
}

/// 非 2xx 响应转成错误；401 / 403 统一归为 Key 问题。
pub(crate) async fn check(resp: reqwest::Response) -> Result<reqwest::Response, ProviderError> {
    let status = resp.status().as_u16();
    if (200..300).contains(&status) {
        return Ok(resp);
    }
    if status == 401 || status == 403 {
        return Err(ProviderError::Auth(status));
    }
    let body = resp.text().await.map_err(net)?;
    Err(ProviderError::Http { status, message: error_message(&body) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages() {
        assert_eq!(error_message(r#"{"error":{"message":"bad"}}"#), "bad");
        assert_eq!(error_message(r#"[{"error":{"code":400,"message":"gem"}}]"#), "gem");
        assert_eq!(error_message(r#"{"error":"plain"}"#), "plain");
        assert_eq!(error_message("  <html>oops</html> "), "<html>oops</html>");
    }
}
