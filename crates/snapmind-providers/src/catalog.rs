//! 内置服务商目录（需求 1.4）：名称、标识色、接口格式、默认地址。

use serde::Serialize;
use snapmind_core::config::ProviderKind;

/// 接口格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiFormat {
    /// OpenAI 兼容（DeepSeek、OpenAI、OpenRouter、Ollama、自定义）
    OpenAi,
    /// Claude 原生
    Claude,
    /// Gemini 原生
    Gemini,
}

/// 服务商的展示信息与默认值。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderMeta {
    pub kind: ProviderKind,
    pub name: &'static str,
    /// 两个字母的标识（列表里的圆角方块）
    pub mono: &'static str,
    pub color: &'static str,
    pub format: ApiFormat,
    pub default_url: &'static str,
    pub needs_key: bool,
    pub desc: &'static str,
}

/// 设置页「添加服务商」里的顺序。
pub const CATALOG: [ProviderMeta; 7] = [
    ProviderMeta {
        kind: ProviderKind::Deepseek,
        name: "DeepSeek",
        mono: "DS",
        color: "#4D6BFE",
        format: ApiFormat::OpenAi,
        default_url: "https://api.deepseek.com/v1",
        needs_key: true,
        desc: "性价比高的中文文本模型",
    },
    ProviderMeta {
        kind: ProviderKind::Anthropic,
        name: "Anthropic",
        mono: "AN",
        color: "#C2410C",
        format: ApiFormat::Claude,
        default_url: "https://api.anthropic.com",
        needs_key: true,
        desc: "Claude 系列，支持图片输入",
    },
    ProviderMeta {
        kind: ProviderKind::Gemini,
        name: "Google Gemini",
        mono: "GE",
        color: "#1A73E8",
        format: ApiFormat::Gemini,
        default_url: "https://generativelanguage.googleapis.com",
        needs_key: true,
        desc: "Gemini 系列，支持图片输入",
    },
    ProviderMeta {
        kind: ProviderKind::Openai,
        name: "OpenAI",
        mono: "OA",
        color: "#10A37F",
        format: ApiFormat::OpenAi,
        default_url: "https://api.openai.com/v1",
        needs_key: true,
        desc: "GPT 系列",
    },
    ProviderMeta {
        kind: ProviderKind::Openrouter,
        name: "OpenRouter",
        mono: "OR",
        color: "#6467F2",
        format: ApiFormat::OpenAi,
        default_url: "https://openrouter.ai/api/v1",
        needs_key: true,
        desc: "一个 Key 调用多家模型",
    },
    ProviderMeta {
        kind: ProviderKind::Ollama,
        name: "Ollama（本地）",
        mono: "OL",
        color: "#334155",
        format: ApiFormat::OpenAi,
        default_url: "http://localhost:11434/v1",
        needs_key: false,
        desc: "在本机运行模型，不需要 Key",
    },
    ProviderMeta {
        kind: ProviderKind::Custom,
        name: "自定义服务",
        mono: "+",
        color: "#64748B",
        format: ApiFormat::OpenAi,
        default_url: "https://",
        needs_key: true,
        desc: "任意 OpenAI 兼容接口",
    },
];

pub fn meta(kind: ProviderKind) -> &'static ProviderMeta {
    CATALOG.iter().find(|m| m.kind == kind).expect("目录覆盖所有种类")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_meta_and_formats_are_right() {
        assert_eq!(meta(ProviderKind::Anthropic).format, ApiFormat::Claude);
        assert_eq!(meta(ProviderKind::Gemini).format, ApiFormat::Gemini);
        assert_eq!(meta(ProviderKind::Ollama).format, ApiFormat::OpenAi);
        assert!(!meta(ProviderKind::Ollama).needs_key);
        assert_eq!(CATALOG.len(), 7);
    }
}
