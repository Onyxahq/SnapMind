//! 大模型（OpenAI 兼容 / Claude / Gemini）与机器翻译（DeepL / Google / 微软）适配器。
//!
//! M5.1：服务商目录、获取模型列表、模型标签推断。流式对话在 M5.2，机器翻译在 M5.3。

pub mod catalog;
pub mod models;

pub use catalog::{meta, ApiFormat, ProviderMeta, CATALOG};
pub use models::{infer_tags, list_models, ProviderError};
