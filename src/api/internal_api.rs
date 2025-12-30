use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use serenity::all::UserId;
use super::server::AppData;
use crate::{api::types::{KhronosValueApi, PublicLuauExecute}, worker::workervmmanager::Id};
use super::extractors::InternalEndpoint;
use super::server::{ApiResponse, ApiError};

/// Dispatch Event
/// 
/// Dispatches a new event under the user account. Note that the `Web` event name restriction does not
/// apply here (along with checking if a guild contains the bot) as this is an internal API.
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
        (status = 204, description = "Event dispatched successfully", body = KhronosValueApi),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn dispatch_event(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    InternalEndpoint { user_id }: InternalEndpoint,
    Path(guild_id): Path<serenity::all::GuildId>,
    Json(req): Json<PublicLuauExecute>,
) -> ApiResponse<KhronosValue> {
    // Make a event
    let user_id: UserId = user_id.parse()
        .map_err(|e: serenity::all::ParseIdError| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    let event = CreateEvent::new_khronos_value(req.name, Some(user_id.to_string()), req.data);

    let resp = data.worker.dispatch_event(
        Id::GuildId(guild_id),
        event,
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(resp))
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

// Template APIs (part of Proposed Unified Templates)

// Set Template State
//
// Sets the state of a template in the template pool
// Possible states are: "active", "paused", "suspended"
//
// A suspended template cannot be used in any guilds until set to
// a different state by staff. Of note, a normal user/guild owner cannot
// change the state of a template they own to "suspended" nor may they
// change the state of a suspended template whatsoever

// Set Template Shop Listing Review State
//
// Sets the review state of a template shop listing
// Possible states are: "pending", "approved", "denied"

// Mock Luau Script