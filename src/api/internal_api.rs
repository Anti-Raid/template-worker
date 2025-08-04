use std::collections::HashMap;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use super::server::AppData;
use crate::{api::types::{ApiLuaVmResult, ApiLuaVmResultHandle}, dispatch::DispatchResult, templatingrt::LuaVmResult};
use crate::templatingrt::cache::regenerate_cache;
use super::types::ExecuteLuaVmActionResponse;
use crate::templatingrt::CreateGuildState;
use crate::templatingrt::POOL;
use crate::templatingrt::MAX_TEMPLATES_RETURN_WAIT_TIME;
use antiraid_types::ar_event::AntiraidEvent;
use super::extractors::InternalEndpoint;
use super::server::ApiResponse;
use super::types::{DispatchEventAndWaitQuery, ExecuteLuaVmActionOpts};
use crate::dispatch::{dispatch, dispatch_and_wait, parse_event};
use crate::templatingrt::execute;

/// Dispatches a new event
pub(super) async fn dispatch_event(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
    Json(event): Json<AntiraidEvent>,
) -> ApiResponse<()> {
    let event = parse_event(&event).map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_string().into())))?;

    dispatch(&serenity_context, &data, event, guild_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(()))
}

/// Dispatches a new event and waits for a response
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
) -> ApiResponse<HashMap<String, DispatchResult<serde_json::Value>>> {
    let event = parse_event(&event).map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_string().into())))?;

    let wait_timeout = match query.wait_timeout {
        Some(timeout) => std::time::Duration::from_millis(timeout),
        None => MAX_TEMPLATES_RETURN_WAIT_TIME,
    };

    let results = dispatch_and_wait(&serenity_context, &data, event, guild_id, wait_timeout)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(results))
}

/// Regenerates the cache for a guild
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

/// Returns the number of threads running
pub(super) async fn get_threads_count(
    State(AppData { .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<usize> {
    let Ok(count) = POOL.len() else {
        return Ok(Json(0));
    };

    Ok(Json(count))
}

/// Ping all threads returning a list of threads which responded
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

/// Flush out inactive guilds
pub(super) async fn clear_inactive_guilds(
    State(AppData { .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<Vec<crate::templatingrt::ThreadClearInactiveGuilds>> {
    let Ok(hm) = crate::templatingrt::POOL.clear_inactive_guilds().await else {
        return Err((
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            Json("Failed to start inactive guild clear".into()),
        ));
    };

    Ok(Json(hm))
}

/// Flush out unused threads
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

/// Closes a thread in the pool
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

/// Execute a lua vm action on a guild
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
    Json(action): Json<crate::templatingrt::LuaVmAction>,
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
        action,
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

/// Get thread pool metrics given tid
#[axum::debug_handler]
pub(super) async fn get_vm_metrics_by_tid(
    State(AppData { ..}): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Path(tid): Path<u64>,
) -> ApiResponse<Vec<crate::templatingrt::ThreadMetrics>> {
    let metrics = crate::templatingrt::POOL
        .get_vm_metrics_by_tid(tid)
        .await
        .map_err(|e| {
            (
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                Json(e.to_string().into()),
            )
        })?;

    Ok(Json(metrics))
}

/// Get thread pool metrics given tid
#[axum::debug_handler]
pub(super) async fn get_vm_metrics_for_all(
    State(AppData { ..}): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<Vec<crate::templatingrt::ThreadMetrics>> {
    let metrics = crate::templatingrt::POOL
        .get_vm_metrics_for_all()
        .await
        .map_err(|e| {
            (
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                Json(e.to_string().into()),
            )
        })?;

    Ok(Json(metrics))
}

/// Given a list of guild ids, return a set of 0s and 1s indicating whether each guild exists in cache
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
