//! 模型服务（需求 1.4）：服务商目录、API Key（只进系统凭据库，绝不回传前端）、获取模型列表。

use snapmind_core::i18n::tr;
use std::collections::HashMap;

use serde::Serialize;
use snapmind_core::config::{ModelTag, ProviderKind};
use snapmind_platform::secrets;
use snapmind_providers::ProviderMeta;

/// 内置服务商目录（设置页「添加服务商」用）。
#[tauri::command]
pub fn provider_catalog() -> Vec<ProviderMeta> {
    // 国内外各有一套地址的服务商，按地区填默认地址
    snapmind_providers::catalog(snapmind_platform::region::mainland_china())
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

/// 用已保存的 Key 获取模型列表，同时就是「测试连接」。服务商在列表里说了能不能看图就听它的，
/// 没说再按模型名推断。
#[tauri::command]
pub async fn provider_fetch_models(id: String, kind: ProviderKind, base_url: String) -> Result<Vec<FetchedModel>, String> {
    let key = secrets::get(&id).map_err(|e| e.to_string())?;
    let list = snapmind_providers::list_models(kind, &base_url, key.as_deref()).await.map_err(|e| e.to_string())?;
    Ok(list.iter().map(|m| FetchedModel { tags: snapmind_providers::tags_for(kind, m), id: m.id.clone() }).collect())
}

/// 测试机器翻译：用已保存的 Key 把一句英文翻成简体中文，返回译文。
#[tauri::command]
pub async fn mt_test(state: tauri::State<'_, crate::state::AppState>, kind: snapmind_core::config::MtKind) -> Result<String, String> {
    let key = secrets::get(kind.secret_account()).map_err(|e| e.to_string())?.ok_or_else(|| tr("请先填写 API Key"))?;
    let region = state.config.get().mt.microsoft_region;
    let ep = snapmind_providers::MtEndpoint { kind, key: &key, region: Some(&region), base_url: None };
    snapmind_providers::translate(&ep, "Hello, world!", Some("en"), "zh-Hans", std::time::Duration::from_secs(15))
        .await
        .map(|r| r.text)
        .map_err(|e| e.to_string())
}
