//! 所有适配器共用的错误类型，消息直接给用户看（也会写进「已切换到……」的说明）。

use serde_json::Value;

#[derive(Debug, Clone, thiserror::Error, PartialEq)]
pub enum ProviderError {
    #[error("请先填写 API Key")]
    MissingKey,
    #[error("API Key 无效或没有权限（HTTP {0}）")]
    Auth(u16),
    #[error("服务返回错误（HTTP {status}）：{message}")]
    Http { status: u16, message: String },
    /// 流式响应中途返回的错误
    #[error("服务返回错误：{0}")]
    Remote(String),
    #[error("内容被模型拒绝：{0}")]
    Refused(String),
    #[error("连接失败：{0}")]
    Network(String),
    #[error("等待响应超时")]
    Timeout,
    #[error("返回内容无法解析：{0}")]
    Parse(String),
    #[error("模型返回了空结果")]
    Empty,
}

impl ProviderError {
    /// 写进「deepseek-chat 超时，已切换到 DeepL」里的短说明。
    pub fn short(&self) -> String {
        match self {
            ProviderError::MissingKey => "没有 Key".into(),
            ProviderError::Auth(_) => "Key 无效".into(),
            ProviderError::Http { status, .. } => format!("出错（HTTP {status}）"),
            ProviderError::Remote(_) => "出错".into(),
            ProviderError::Refused(_) => "拒绝回答".into(),
            ProviderError::Network(_) => "连不上".into(),
            ProviderError::Timeout => "超时".into(),
            ProviderError::Parse(_) => "返回格式不对".into(),
            ProviderError::Empty => "返回空结果".into(),
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
