use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use super::server::AppData;
use crate::{api::types::{ApiDispatchResult, ApiGuildId, ApiLuaVmAction, ApiLuaVmResult, ApiLuaVmResultHandle, ApiThreadClearInactiveGuilds, ApiThreadMetrics}, templatingrt::LuaVmResult};
use crate::templatingrt::cache::regenerate_cache;
use super::types::ExecuteLuaVmActionResponse;
use crate::templatingrt::CreateGuildState;
use crate::templatingrt::POOL;
use crate::templatingrt::MAX_TEMPLATES_RETURN_WAIT_TIME;
use crate::events::AntiraidEvent;
use super::extractors::InternalEndpoint;
use super::server::{ApiResponse, ApiError};
use super::types::{DispatchEventAndWaitQuery, ExecuteLuaVmActionOpts};
use crate::dispatch::{dispatch, dispatch_and_wait, parse_event};
use crate::templatingrt::execute;

/// Dispatch Event
/// 
/// Dispatches a new event
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/dispatch-event/{guild_id}",
    responses(
        (status = 204, description = "Event dispatched successfully"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn dispatch_event(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint,
    Path(guild_id): Path<serenity::all::GuildId>,
    Json(event): Json<AntiraidEvent>,
) -> ApiResponse<()> {
    let event = parse_event(&event).map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_string().into())))?;

    dispatch(&serenity_context, &data, event, guild_id)
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
    responses(
        (status = 200, description = "Event dispatched successfully", body = DispatchResponse),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn dispatch_event_and_wait(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
    Query(query): Query<DispatchEventAndWaitQuery>,
    Json(event): Json<AntiraidEvent>,
) -> ApiResponse<DispatchResponse> {
    let event = parse_event(&event).map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_string().into())))?;

    let wait_timeout = match query.wait_timeout {
        Some(timeout) => std::time::Duration::from_millis(timeout),
        None => MAX_TEMPLATES_RETURN_WAIT_TIME,
    };

    let results = dispatch_and_wait(&serenity_context, &data, event, guild_id, wait_timeout)
        .await
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
    responses(
        (status = 204, description = "Cache regenerated successfully"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn regenerate_cache_api(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
) -> ApiResponse<()> {
    regenerate_cache(&serenity_context, &data, guild_id)
        .await
        .map_err(|e| (
            StatusCode::INTERNAL_SERVER_ERROR, Json(
                format!("Failed to regenerate cache: {e:?}").into()
            )
        ))?;
    Ok(Json(()))
}

/// Get Thread Count
///
/// Returns the number of threads running
#[utoipa::path(
    get, 
    tag = "Internal API",
    path = "/i/threads-count",
    responses(
        (status = 200, description = "The number of threads", body = usize),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_threads_count(
    State(AppData { .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<usize> {
    let Ok(count) = POOL.len() else {
        return Ok(Json(0));
    };

    Ok(Json(count))
}

/// Ping All Threads
/// 
/// Ping all threads returning a list of threads which responded
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/ping-all-threads",
    responses(
        (status = 200, description = "The number of threads", body = Vec<u64>),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn ping_all_threads(
    State(AppData { .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<Vec<u64>> {
    let Ok(hm) = crate::templatingrt::POOL.ping().await else {
        return Err((
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            Json("Failed to start ping".into()),
        ));
    };

    Ok(Json(hm))
}

/// Clear Inactive Guilds
/// 
/// Flush out inactive guilds
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/clear-inactive-guilds",
    responses(
        (status = 200, description = "The cleared guilds data", body = Vec<ApiThreadClearInactiveGuilds>),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn clear_inactive_guilds(
    State(AppData { .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<Vec<ApiThreadClearInactiveGuilds>> {
    let Ok(hm) = crate::templatingrt::POOL.clear_inactive_guilds().await else {
        return Err((
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            Json("Failed to start inactive guild clear".into()),
        ));
    };

    let hm = hm.into_iter()
        .map(|data| ApiThreadClearInactiveGuilds {
            tid: data.tid,
            cleared: data.cleared,
        })
        .collect::<Vec<_>>();

    Ok(Json(hm))
}

/// Remove Unused Threads
/// 
/// Remove unused threads from the thread pool returning a list of thread IDs of the removed threads
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/remove-unused-threads",
    responses(
        (status = 200, description = "The list of thread IDs of the removed threads", body = Vec<u64>),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn remove_unused_threads(
    State(AppData { .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<Vec<u64>> {
    let Ok(hm) = crate::templatingrt::POOL.remove_unused_threads().await else {
        return Err((
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            Json("Failed to start unused thread clear".into()),
        ));
    };

    Ok(Json(hm))
}

/// Close Thread
/// 
/// Closes a thread in the pool
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/close-thread",
    responses(
        (status = 204, description = "The ID of the thread that was closed"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
#[axum::debug_handler]
pub(super) async fn close_thread(
    State(AppData { .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(tid): Path<u64>,
) -> ApiResponse<()> {
    crate::templatingrt::POOL
        .close_thread(tid)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(()))
}

/// Execute Lua VM Action
/// 
/// Execute a Lua VM action on a guild
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/execute-luavmaction",
    responses(
        (status = 200, description = "The response from the thread regarding the operation", body = ExecuteLuaVmActionResponse),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
#[axum::debug_handler]
pub(super) async fn execute_lua_vm_action(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
    Query(opts): Query<ExecuteLuaVmActionOpts>,
    Json(action): Json<ApiLuaVmAction>,
) -> ApiResponse<ExecuteLuaVmActionResponse> {
    let start_instant = std::time::Instant::now();
    let handle = execute(
        guild_id,
        CreateGuildState {
            pool: data.pool.clone(),
            serenity_context,
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
        },
        action.into(),
    )
    .await
    .map_err(|e| {
        (
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            Json(e.to_string().into()),
        )
    })?;

    let result_handle = handle
        .wait_timeout(opts.wait_timeout.unwrap_or(MAX_TEMPLATES_RETURN_WAIT_TIME))
        .await
        .map_err(|e| {
            (
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                Json(e.to_string().into()),
            )
        })?;

    // Convert results to API format
    let mut results_api = Vec::with_capacity(result_handle.results.len());

    for result in result_handle.results {
        let api_result = ApiLuaVmResultHandle {
            template_name: result.template_name,
            result: match result.result {
                LuaVmResult::Ok { result_val } => ApiLuaVmResult::Ok { result: result_val },
                LuaVmResult::LuaError { err } => ApiLuaVmResult::LuaError { err },
                LuaVmResult::VmBroken { } => ApiLuaVmResult::VmBroken { },
            }
        };
        results_api.push(api_result);
    }

    let elapsed = start_instant.elapsed();

    Ok(Json(ExecuteLuaVmActionResponse {
        results: results_api,
        time_taken: elapsed,
    }))
}

/// Get VM Metrics By TID
/// 
/// Get thread pool metrics given Thread ID
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/get-vm-metrics-by-tid/{tid}",
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

/// Guilds Exist
/// 
/// Given a list of guild ids, return a set of 0s and 1s indicating whether each guild exists in cache
#[utoipa::path(
    post, 
    tag = "Internal API",
    path = "/i/guilds-exist",
    responses(
        (status = 200, description = "The list of which guilds exist", body = Vec<u8>),
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
    Json(guilds): Json<Vec<ApiGuildId>>,
) -> ApiResponse<Vec<u8>> {
    let guilds_exist = crate::sandwich::has_guilds(
        &data.reqwest,
        guilds.into_iter().map(|g| g.into()).collect::<Vec<_>>(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(guilds_exist))
}
