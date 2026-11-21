//! 大模型（OpenAI 兼容 / Claude / Gemini）与机器翻译（DeepL / Google / 微软）适配器。
//!
//! M5 开始实现。

/// 支持的大模型接口格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiFormat {
    OpenAiCompatible,
    Claude,
    Gemini,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_are_distinct() {
        assert_ne!(ApiFormat::OpenAiCompatible, ApiFormat::Claude);
        assert_ne!(ApiFormat::Claude, ApiFormat::Gemini);
    }
}
