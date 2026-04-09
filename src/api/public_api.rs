use crate::api::gkv::PartialGlobalKv;
use crate::api::types::ApiConfig;
use crate::api::types::GetStatusResponse;
use crate::api::types::PartialGlobalKvList;
use crate::api::types::ShardConn;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum::Json;
use moka::future::Cache;
use std::sync::Arc;

use super::types::{
    TwState,
};
use super::server::{AppData, ApiResponse, ApiError}; 

static STATE_CACHE: std::sync::LazyLock<Arc<TwState>> = std::sync::LazyLock::new(|| {    
    let state = TwState {
        commands: crate::register::REGISTER.commands.clone(),
    };

    Arc::new(state)
});

/// Get Bot State
/// 
/// Returns the list of core/builtin commands of the bot
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/bot-state",
    responses(
        (status = 200, description = "The bot's state", body = TwState),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn state() -> Json<Arc<TwState>> {
    Json(STATE_CACHE.clone())
}

/// Get API Configuration
/// 
/// Returns the base API configuration
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/config",
    responses(
        (status = 200, description = "The base API configuration", body = ApiConfig),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn api_config() -> Json<ApiConfig> {
    Json(ApiConfig {
        main_server: crate::CONFIG.servers.main,
        client_id: crate::CONFIG.discord_auth.client_id,
        support_server_invite: 
        crate::CONFIG.meta.support_server_invite.clone(),
    })
}

static STATS_CACHE: std::sync::LazyLock<Cache<(), GetStatusResponse>> = std::sync::LazyLock::new(|| {
    Cache::builder()
        .time_to_live(std::time::Duration::from_secs(100)) // 1 minute
        .build()
});

/// Get Bot Stats
/// 
/// Returns the bot's stats
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/bot-stats",
    responses(
        (status = 200, description = "The bot's state", body = GetStatusResponse),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_bot_stats(
    State(AppData { stratum, .. }): State<AppData>,
) -> ApiResponse<GetStatusResponse> {
    let stats = STATS_CACHE.try_get_with::<_, crate::Error>((), async move {
        let raw_stats = stratum.get_status().await?;

        let stats = GetStatusResponse {
            shard_conns: raw_stats.shards.into_iter().map(|shard| {
                (shard.shard_id, ShardConn {
                    status: shard.state().as_str_name().to_string(),
                    latency: shard.latency,
                })
            }).collect(),
            total_guilds: raw_stats.guild_count,
            total_users: raw_stats.user_count,
        };

        Ok(stats)
    }).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to get bot stats: {e:?}").into())))?;

    Ok(Json(stats))
}

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
    let items = gkv.global_kv_find(params.scope, params.query.unwrap_or_else(|| "%".to_string()))
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
