//! 大模型（OpenAI 兼容 / Claude / Gemini）与机器翻译（DeepL / Google / 微软）适配器。
//!
//! - catalog：内置服务商目录；models：获取模型列表、标签推断（M5.1）
//! - chat：三种接口的流式对话，文本和图片输入（M5.2）
//! - mt：DeepL、Google、微软机器翻译（M5.3）
//! - chain：备用链执行器（M5.4）
//! - error：统一的错误类型；sse：流式响应解析

pub mod catalog;
pub mod chain;
pub mod chat;
pub mod error;
pub mod models;
pub mod mt;
pub mod sse;

pub use chain::{describe_failures, mt_name, resolve, run_text_chain, Backend, ChainEvent, ChainRun, ChainSuccess, Failure, TextJob};
pub use catalog::{meta, ApiFormat, ProviderMeta, CATALOG};
pub use chat::{stream_chat, ChatRequest, Endpoint, Message, Part, Role};
pub use error::ProviderError;
pub use models::{infer_tags, list_models};
pub use mt::{translate, MtEndpoint, MtOutput};
