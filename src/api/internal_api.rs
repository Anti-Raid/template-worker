use std::collections::HashMap;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use uuid::Uuid;
use super::server::AppData;
use crate::{api::types::ApiDispatchResult, dispatch::parse_response, templatedb::base_template::{BaseTemplate, BaseTemplateRef}, worker::workervmmanager::Id};
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

// Template APIs (part of Proposed Unified Templates)

/// Fetch All Templates in Pool
/// 
/// Fetches all templates in the template pool
#[utoipa::path(
    get, 
    tag = "Internal API",
    path = "/i/templates/fetch_all_templates_in_pool",
    security(
        ("InternalAuth" = []) 
    ),
    responses(
        (status = 200, description = "The list of all templates in the template pool"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn fetch_all_templates_in_pool(
    State(AppData { data, .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<Vec<BaseTemplate>> {
    let templates = BaseTemplate::fetch_all(&data.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(templates))
}

/// Fetch Template in Pool by ID
///
/// Fetches a template in the template pool by ID. Returns `null` if not found, otherwise BaseTemplate.
#[utoipa::path(
    get, 
    tag = "Internal API",
    path = "/i/templates/fetch_templates_in_pool_by_id/{id}",
    security(
        ("InternalAuth" = []) 
    ),
    responses(
        (status = 200, description = "The template in the template pool matching the ID or `null` if not found"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn fetch_template_in_pool_by_id(
    State(AppData { data, .. }): State<AppData>,
    Path(id): Path<Uuid>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
) -> ApiResponse<Option<BaseTemplate>> {
    let bref = BaseTemplateRef::new(id);
    let templates = bref.fetch_from_db(&data.pool)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(templates))
}

/// Fetch Templates in Pool by IDs
///
/// Fetches multiple templates in the template pool by their IDs
///
/// The response is a list of templates whose IDs matched the requested IDs.
#[utoipa::path(
    get, 
    tag = "Internal API",
    path = "/i/templates/fetch_templates_in_pool_by_ids",
    security(
        ("InternalAuth" = []) 
    ),
    request_body = Vec<Uuid>,
    responses(
        (status = 200, description = "The list of the specified templates in the template pool"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
#[axum::debug_handler]
pub(super) async fn fetch_template_in_pool_by_ids(
    State(AppData { data, .. }): State<AppData>,
    InternalEndpoint { .. }: InternalEndpoint, // Internal endpoint
    Json(ids): Json<Vec<Uuid>>,
) -> ApiResponse<Vec<BaseTemplate>> {
    let templates = BaseTemplate::fetch_by_ids(&data.pool, ids)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    Ok(Json(templates))
}

// Set Template State
//
// Sets the state of a template in the template pool
// Possible states are: "active", "paused", "suspended"
//
// A suspended template cannot be used in any guilds until set to
// a different state by staff. Of note, a normal user/guild owner cannot
// change the state of a template they own to "suspended" nor may they
// change the state of a suspended template whatsoever

// Fetch Template Shop Listings
//
// Fetches all template shop listings

// Fetches Template Shop Listings by ID
//
// Fetches a template shop listing by ID.
// The ID here is the same as the base template ID it references.
// This is to ensure that there is a one-to-one mapping between a base 
// template and its shop listing (without needing an extra ID field).

// Set Template Shop Listing Review State
//
// Sets the review state of a template shop listing
// Possible states are: "pending", "approved", "denied"

// Fetch All Attached Templates
//
// Fetches all attached templates
//
// Note that the information returned by this API does not include the full
// base template information, only the attached template information.

// Fetch Attached Templates For Owner
//
// Fetches all attached templates for a given owner ID
//
// Note that the information returned by this API does not include the full
// base template information, only the attached template information.

// Delete Attached Template
//
// Deletes an attached template for a given owner ID and base template ID