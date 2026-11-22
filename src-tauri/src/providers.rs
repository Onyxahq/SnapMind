//! 模型服务（需求 1.4）：服务商目录、API Key（只进系统凭据库，绝不回传前端）、获取模型列表。

use std::collections::HashMap;

use serde::Serialize;
use snapmind_core::config::{ModelTag, ProviderKind};
use snapmind_platform::secrets;
use snapmind_providers::ProviderMeta;

/// 内置服务商目录（设置页「添加服务商」用）。
#[tauri::command]
pub fn provider_catalog() -> Vec<ProviderMeta> {
    snapmind_providers::CATALOG.to_vec()
}

/// 哪些服务商已经保存了 Key（只回答有没有，不回传内容）。
#[tauri::command]
pub fn provider_key_status(ids: Vec<String>) -> HashMap<String, bool> {
    ids.into_iter()
        .map(|id| {
            let has = secrets::get(&id).ok().flatten().is_some();
            (id, has)
        })
        .collect()
}

/// 保存 Key；空字符串表示删除（移除服务商时也调用）。
#[tauri::command]
pub fn provider_set_key(id: String, key: String) -> Result<(), String> {
    secrets::set(&id, &key).map_err(|e| e.to_string())
}

#[derive(Debug, Serialize)]
pub struct FetchedModel {
    pub id: String,
    pub tags: Vec<ModelTag>,
}

/// 用已保存的 Key 获取模型列表，同时就是「测试连接」。标签按模型名推断。
#[tauri::command]
pub async fn provider_fetch_models(id: String, kind: ProviderKind, base_url: String) -> Result<Vec<FetchedModel>, String> {
    let key = secrets::get(&id).map_err(|e| e.to_string())?;
    let ids = snapmind_providers::list_models(kind, &base_url, key.as_deref()).await.map_err(|e| e.to_string())?;
    Ok(ids
        .into_iter()
        .map(|m| FetchedModel { tags: snapmind_providers::infer_tags(kind, &m), id: m })
        .collect())
}
