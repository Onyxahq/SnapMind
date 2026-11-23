//! 内置服务商目录（需求 1.4）：名称、标识色、接口格式、默认地址。
//!
//! 国内外各有一套地址的服务商（Kimi、智谱、通义、硅基流动、MiniMax、阶跃星辰）存两个地址，
//! 按用户所在地区选默认的那个，另一个作为提示显示在 Base URL 下面。

use serde::Serialize;
use snapmind_core::config::ProviderKind;

/// 接口格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ApiFormat {
    /// OpenAI 兼容（除 Claude、Gemini 外的所有内置服务商和自定义服务）
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
    /// 海外（国际站）地址；只有国内站的服务商就是国内地址
    pub default_url: &'static str,
    /// 中国大陆站的地址（和国际站不同时）
    #[serde(skip)]
    pub cn_url: Option<&'static str>,
    /// 另一个地区的地址（由 [`catalog`] 按地区填）
    pub alt_url: Option<&'static str>,
    pub needs_key: bool,
    pub desc: &'static str,
}

const fn p(
    kind: ProviderKind,
    name: &'static str,
    mono: &'static str,
    color: &'static str,
    default_url: &'static str,
    cn_url: Option<&'static str>,
    desc: &'static str,
) -> ProviderMeta {
    ProviderMeta { kind, name, mono, color, format: ApiFormat::OpenAi, default_url, cn_url, alt_url: None, needs_key: true, desc }
}

use ProviderKind as K;

/// 设置页「添加服务商」里的顺序：常用的在前，本机服务和自定义在最后。
pub const CATALOG: [ProviderMeta; 24] = [
    p(K::Deepseek, "DeepSeek", "DS", "#4D6BFE", "https://api.deepseek.com/v1", None, "性价比高的中文文本模型"),
    ProviderMeta {
        format: ApiFormat::Claude,
        ..p(K::Anthropic, "Anthropic", "AN", "#C2410C", "https://api.anthropic.com", None, "Claude 系列，支持图片输入")
    },
    ProviderMeta {
        format: ApiFormat::Gemini,
        ..p(K::Gemini, "Google Gemini", "GE", "#1A73E8", "https://generativelanguage.googleapis.com", None, "Gemini 系列，支持图片输入")
    },
    p(K::Openai, "OpenAI", "OA", "#10A37F", "https://api.openai.com/v1", None, "GPT 系列，GPT-4o 起支持图片输入"),
    p(K::Openrouter, "OpenRouter", "OR", "#6467F2", "https://openrouter.ai/api/v1", None, "一个 Key 调用多家模型，能不能看图按它的模型信息标注"),
    p(
        K::Qwen,
        "通义千问",
        "QW",
        "#615CED",
        "https://dashscope-intl.aliyuncs.com/compatible-mode/v1",
        Some("https://dashscope.aliyuncs.com/compatible-mode/v1"),
        "阿里云百炼，Qwen-VL / Qwen3.5 起支持图片输入",
    ),
    p(
        K::Moonshot,
        "Kimi",
        "KI",
        "#16191E",
        "https://api.moonshot.ai/v1",
        Some("https://api.moonshot.cn/v1"),
        "月之暗面，K2.5 起支持图片输入",
    ),
    p(
        K::Zhipu,
        "智谱 GLM",
        "GL",
        "#3859FF",
        "https://api.z.ai/api/paas/v4",
        Some("https://open.bigmodel.cn/api/paas/v4"),
        "GLM 系列，GLM-4.5V / 4.6V 支持图片输入",
    ),
    p(
        K::Doubao,
        "火山方舟",
        "DB",
        "#1664FF",
        "https://ark.cn-beijing.volces.com/api/v3",
        None,
        "豆包系列，Seed 1.6 起支持图片输入",
    ),
    p(
        K::Siliconflow,
        "硅基流动",
        "SF",
        "#7C3AED",
        "https://api.siliconflow.com/v1",
        Some("https://api.siliconflow.cn/v1"),
        "托管多家开源模型（Qwen、GLM、DeepSeek……）",
    ),
    p(K::Xai, "xAI", "XA", "#111827", "https://api.x.ai/v1", None, "Grok 系列，Grok 4 起支持图片输入"),
    p(K::Mistral, "Mistral", "MI", "#FA520F", "https://api.mistral.ai/v1", None, "Mistral 系列，Pixtral / Small 3.1 起支持图片输入"),
    p(K::Minimax, "MiniMax", "MM", "#E2492F", "https://api.minimax.io/v1", Some("https://api.minimaxi.com/v1"), "MiniMax 文本模型"),
    p(K::Stepfun, "阶跃星辰", "SP", "#0EA5E9", "https://api.stepfun.ai/v1", Some("https://api.stepfun.com/v1"), "Step 系列，Step-1V / Step 3 支持图片输入"),
    p(K::Qianfan, "百度千帆", "QF", "#2932E1", "https://qianfan.baidubce.com/v2", None, "文心 ERNIE 系列，VL 型号支持图片输入"),
    p(K::Hunyuan, "腾讯混元", "HY", "#0052D9", "https://api.hunyuan.cloud.tencent.com/v1", None, "混元系列，Vision 型号支持图片输入"),
    p(K::Modelscope, "魔搭社区", "MS", "#624AFF", "https://api-inference.modelscope.cn/v1", None, "魔搭 API-Inference，托管开源模型，每天有免费额度"),
    p(K::Groq, "Groq", "GQ", "#F55036", "https://api.groq.com/openai/v1", None, "速度很快的开源模型推理"),
    p(K::Together, "Together AI", "TG", "#0F6FFF", "https://api.together.xyz/v1", None, "托管多家开源模型"),
    p(K::Fireworks, "Fireworks", "FW", "#6720FF", "https://api.fireworks.ai/inference/v1", None, "托管多家开源模型"),
    p(K::Nvidia, "NVIDIA NIM", "NV", "#76B900", "https://integrate.api.nvidia.com/v1", None, "NVIDIA 托管的开源模型，有免费额度"),
    ProviderMeta { needs_key: false, ..p(K::Ollama, "Ollama（本地）", "OL", "#334155", "http://localhost:11434/v1", None, "在本机运行模型，不需要 Key") },
    ProviderMeta {
        needs_key: false,
        ..p(K::Lmstudio, "LM Studio（本地）", "LM", "#4338CA", "http://localhost:1234/v1", None, "在本机运行模型，先在 LM Studio 里开启本地服务")
    },
    p(K::Custom, "自定义服务", "+", "#64748B", "https://", None, "任意 OpenAI 兼容接口"),
];

pub fn meta(kind: ProviderKind) -> &'static ProviderMeta {
    CATALOG.iter().find(|m| m.kind == kind).expect("目录覆盖所有种类")
}

/// 给设置页的目录：有国内站的服务商，中国大陆用户默认填国内地址，另一个放在 `alt_url`。
pub fn catalog(mainland_china: bool) -> Vec<ProviderMeta> {
    CATALOG
        .iter()
        .map(|m| {
            let mut m = m.clone();
            if let Some(cn) = m.cn_url {
                m.alt_url = Some(if mainland_china { m.default_url } else { cn });
                if mainland_china {
                    m.default_url = cn;
                }
            }
            m
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_has_meta_and_formats_are_right() {
        assert_eq!(meta(ProviderKind::Anthropic).format, ApiFormat::Claude);
        assert_eq!(meta(ProviderKind::Gemini).format, ApiFormat::Gemini);
        assert_eq!(meta(ProviderKind::Ollama).format, ApiFormat::OpenAi);
        assert_eq!(meta(ProviderKind::Moonshot).format, ApiFormat::OpenAi);
        assert!(!meta(ProviderKind::Ollama).needs_key && !meta(ProviderKind::Lmstudio).needs_key);
        assert!(meta(ProviderKind::Zhipu).needs_key);
        assert_eq!(CATALOG.len(), 24);
        // 每个种类只出现一次
        let kinds: std::collections::HashSet<_> = CATALOG.iter().map(|m| m.kind).collect();
        assert_eq!(kinds.len(), CATALOG.len());
    }

    #[test]
    fn region_picks_default_url() {
        let cn = catalog(true);
        let kimi = cn.iter().find(|m| m.kind == ProviderKind::Moonshot).unwrap();
        assert_eq!((kimi.default_url, kimi.alt_url), ("https://api.moonshot.cn/v1", Some("https://api.moonshot.ai/v1")));
        let intl = catalog(false);
        let kimi = intl.iter().find(|m| m.kind == ProviderKind::Moonshot).unwrap();
        assert_eq!((kimi.default_url, kimi.alt_url), ("https://api.moonshot.ai/v1", Some("https://api.moonshot.cn/v1")));
        let ds = intl.iter().find(|m| m.kind == ProviderKind::Deepseek).unwrap();
        assert_eq!(ds.alt_url, None);
        // 序列化成前端要的字段，cn_url 不出现
        let v = serde_json::to_value(kimi).unwrap();
        assert!(v.get("altUrl").is_some() && v.get("cnUrl").is_none() && v.get("defaultUrl").is_some());
    }
}
