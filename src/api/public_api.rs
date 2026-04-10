use crate::api::gkv::PartialGlobalKv;
use crate::api::types::PartialGlobalKvList;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum::Json;

use super::server::{AppData, ApiResponse, ApiError}; 

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub(super) struct ListGlobalKvParams {
    scope: String,
    query: Option<String>,
}

/// List Global KV
/// 
/// Lists the global KV entries 
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/global-kvs",
    params(
        ("scope" = String, Query, description = "Scope to use for filtering"),
        ("query" = Option<String>, Query, description = "Optional query to filter keys. Defaults to '%%' which lists all keys.")
    ),
    responses(
        (status = 200, description = "The global kv listing", body = PartialGlobalKvList),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn list_global_kv(
    State(AppData { gkv, .. }): State<AppData>,
    Query(params): Query<ListGlobalKvParams>,
) -> ApiResponse<PartialGlobalKvList> {
    let items = gkv.global_kv_find(params.scope, params.query.unwrap_or_else(|| "%%".to_string()))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to list global kvs: {e:?}").into())))?;

    Ok(Json(PartialGlobalKvList { items }))
}

/// Get Global KV by Key-Version
/// 
/// Gets the data for a template shop listing
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/global-kvs/{scope}/{key}/{version}",
    params(
        ("key" = String, description = "The key of the global kv to get"),
        ("version" = String, description = "The version of the global kv to get")
    ),
    responses(
        (status = 200, description = "The global kv", body = PartialGlobalKv),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_global_kv(
    State(AppData { gkv, .. }): State<AppData>,
    Path((scope, key, version)): Path<(String, String, i32)>,
) -> ApiResponse<PartialGlobalKv> {
    let item = gkv.global_kv_get(key, version, scope)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to get global kv: {e:?}").into())))?;

        match item {
        Some(item) => {
            Ok(Json(item))
        },
        None => Err((StatusCode::NOT_FOUND, Json("Global KV not found".into()))),
    }
}
