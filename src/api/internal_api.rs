use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use super::server::AppData;
use crate::{api::types::ApiDispatchResult, dispatch::parse_response, worker::workervmmanager::Id};
use crate::events::AntiraidEvent;
use super::extractors::InternalEndpoint;
use super::server::{ApiResponse, ApiError};
use crate::dispatch::parse_event;

/// Dispatch Event
/// 
/// Dispatches a new event
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/dispatch-event/{guild_id}",
    security(
        ("InternalAuth" = []) 
    ),
    params(
        ("guild_id" = String, description = "The ID of the guild to dispatch the event to")
    ),
    responses(
        (status = 204, description = "Event dispatched successfully"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn dispatch_event(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint,
    Path(guild_id): Path<serenity::all::GuildId>,
    Json(event): Json<AntiraidEvent>,
) -> ApiResponse<()> {
    let event = parse_event(&event).map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_string().into())))?;

    data.worker.dispatch_event_to_templates_nowait(Id::GuildId(guild_id), event)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(()))
}

type DispatchResponse = HashMap<String, ApiDispatchResult<serde_json::Value>>;

/// Dispatch Event And Wait
/// 
/// Dispatches a new event and waits for a response
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/dispatch-event/{guild_id}/@wait",
    security(
        ("InternalAuth" = []) 
    ),
    params(
        ("guild_id" = String, description = "The ID of the guild to dispatch the event to")
    ),
    responses(
        (status = 200, description = "Event dispatched successfully", body = DispatchResponse),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn dispatch_event_and_wait(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
    Json(event): Json<AntiraidEvent>,
) -> ApiResponse<DispatchResponse> {
    let event = parse_event(&event).map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_string().into())))?;

    let results = parse_response(
        data.worker.dispatch_event_to_templates(Id::GuildId(guild_id), event)
        .await
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?
    .into_iter()
    .map(|(name, result)| (name, result.into()))
    .collect::<HashMap<_, _>>();

    Ok(Json(results))
}

/// Regenerate Guild Cache
///
/// Regenerates the cache for a guild
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/regenerate-cache/{guild_id}",
    security(
        ("InternalAuth" = []) 
    ),
    params(
        ("guild_id" = String, description = "The ID of the guild to regenerate the cache for")
    ),
    responses(
        (status = 204, description = "Cache regenerated successfully"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn regenerate_cache_api(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
) -> ApiResponse<()> {
    data.worker.regenerate_cache(Id::GuildId(guild_id))
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(()))
}

/// Get Thread Count
///
/// Returns the number of threads running
#[utoipa::path(
    get, 
    tag = "Internal API",
    path = "/i/threads-count",
    security(
        ("InternalAuth" = []) 
    ),
    responses(
        (status = 200, description = "The number of threads", body = usize),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_threads_count(
    State(AppData { data, .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<usize> {
    Ok(Json(data.worker.len()))
}

/*
/// Get VM Metrics By TID
/// 
/// Get thread pool metrics given Thread ID
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/get-vm-metrics-by-tid/{tid}",
    security(
        ("InternalAuth" = []) 
    ),
    responses(
        (status = 200, description = "The list of all thread metrics where the thread ID matches the VM", body = Vec<ApiThreadMetrics>),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
#[axum::debug_handler]
pub(super) async fn get_vm_metrics_by_tid(
    State(AppData { ..}): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(tid): Path<u64>,
) -> ApiResponse<Vec<ApiThreadMetrics>> {
    let metrics = crate::templatingrt::POOL
        .get_vm_metrics_by_tid(tid)
        .await
        .map_err(|e| {
            (
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                Json(e.to_string().into()),
            )
        })?
        .into_iter()
        .map(ApiThreadMetrics::from)
        .collect::<Vec<_>>();

    Ok(Json(metrics))
}

/// Get All VM Metrics
/// 
/// Get all thread pool metrics
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/get-vm-metrics-for-all",
    security(
        ("InternalAuth" = []) 
    ),
    responses(
        (status = 200, description = "The list of all thread metrics", body = Vec<ApiThreadMetrics>),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
#[axum::debug_handler]
pub(super) async fn get_vm_metrics_for_all(
    State(AppData { ..}): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<Vec<ApiThreadMetrics>> {
    let metrics = crate::templatingrt::POOL
        .get_vm_metrics_for_all()
        .await
        .map_err(|e| {
            (
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                Json(e.to_string().into()),
            )
        })?
        .into_iter()
        .map(ApiThreadMetrics::from)
        .collect::<Vec<_>>();

    Ok(Json(metrics))
}
    */

/// Guilds Exist
/// 
/// Given a list of guild ids, return a set of 0s and 1s indicating whether each guild exists in cache
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/guilds-exist",
    security(
        ("InternalAuth" = []) 
    ),
    request_body = Vec<String>,
    responses(
        (status = 200, description = "The list of which guilds exist where 0 means not existing and 1 means existing in cache", body = Vec<u8>),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
#[axum::debug_handler]
pub(super) async fn guilds_exist(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Json(guilds): Json<Vec<serenity::all::GuildId>>,
) -> ApiResponse<Vec<u8>> {
    let guilds_exist = crate::sandwich::has_guilds(
        &data.reqwest,
        guilds,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(guilds_exist))
}

/// Kill Worker
/// 
/// Kill a worker thread. Note that kills the WorkerLike object and so if run on a WorkerPool, 
/// it will kill all workers in the pool. Only useful for debugging purposes.
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/kill-worker",
    security(
        ("InternalAuth" = []) 
    ),
    request_body = Vec<String>,
    responses(
        (status = 200, description = "Killed worker successfully"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
#[axum::debug_handler]
pub(super) async fn kill_worker(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<()> {
    data.worker.kill()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(()))
}