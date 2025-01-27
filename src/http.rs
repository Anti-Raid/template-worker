use crate::templatingrt::{
    benchmark_vm as benchmark_vm_impl, cache::regenerate_cache, FireBenchmark,
    MAX_TEMPLATES_RETURN_WAIT_TIME,
};
use antiraid_types::ar_event::AntiraidEvent;
use ar_settings::types::OperationType;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use crate::dispatch::{dispatch, dispatch_and_wait, parse_event};

#[derive(Clone)]
pub struct AppData {
    pub data: Arc<silverpelt::data::Data>,
    pub serenity_context: serenity::all::Context,
}

impl AppData {
    pub fn new(data: Arc<silverpelt::data::Data>, ctx: &serenity::all::Context) -> Self {
        Self {
            data,
            serenity_context: ctx.clone(),
        }
    }
}

type Response<T> = Result<Json<T>, (StatusCode, String)>;

pub fn create(
    data: Arc<silverpelt::data::Data>,
    ctx: &serenity::all::Context,
) -> axum::routing::IntoMakeService<Router> {
    let router = Router::new()
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .route("/dispatch-event/:guild_id", post(dispatch_event))
        .route(
            "/dispatch-event/:guild_id/@wait",
            post(dispatch_event_and_wait),
        )
        .route("/benchmark-vm/:guild_id", post(benchmark_vm))
        .route("/pages/:guild_id", post(get_pages_for_guild))
        .route(
            "/page-settings-operation/:guild_id/:user_id",
            post(settings_operation),
        );
    let router: Router<()> = router.with_state(AppData::new(data, ctx));
    router.into_make_service()
}

/// Dispatches a new event
async fn dispatch_event(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    Path(guild_id): Path<serenity::all::GuildId>,
    Json(event): Json<AntiraidEvent>,
) -> Response<()> {
    // Regenerate cache for guild if event is OnStartup
    if let AntiraidEvent::OnStartup(_) = event {
        regenerate_cache(guild_id, &data.pool).await;
    }

    let event = parse_event(&event).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    dispatch(&serenity_context, &data, event, guild_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(()))
}

/// Query parameters for dispatch_event_and_wait
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DispatchEventAndWaitQuery {
    /// Wait duration in milliseconds
    pub wait_timeout: Option<u64>,
}

/// Dispatches a new event and waits for a response
async fn dispatch_event_and_wait(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    Path(guild_id): Path<serenity::all::GuildId>,
    Query(query): Query<DispatchEventAndWaitQuery>,
    Json(event): Json<AntiraidEvent>,
) -> Response<Vec<serde_json::Value>> {
    // Regenerate cache for guild if event is OnStartup
    if let AntiraidEvent::OnStartup(_) = event {
        regenerate_cache(guild_id, &data.pool).await;
    }

    let event = parse_event(&event).map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

    let wait_timeout = match query.wait_timeout {
        Some(timeout) => std::time::Duration::from_millis(timeout),
        None => MAX_TEMPLATES_RETURN_WAIT_TIME,
    };

    let results = dispatch_and_wait(&serenity_context, &data, event, guild_id, wait_timeout)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(results))
}

/// Benchmarks a VM
async fn benchmark_vm(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    Path(guild_id): Path<serenity::all::GuildId>,
) -> Response<FireBenchmark> {
    let bvm = benchmark_vm_impl(
        guild_id,
        data.pool.clone(),
        serenity_context,
        data.reqwest.clone(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(bvm))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsOperationRequest {
    pub fields: indexmap::IndexMap<String, Value>,
    pub op: OperationType,
    pub template: String,
    pub setting_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CanonicalSettingsResult {
    Ok {
        fields: Vec<indexmap::IndexMap<String, Value>>,
    },
    Err {
        error: String,
    },
}

/// Gets the pages for a guild
pub(crate) async fn get_pages_for_guild(
    State(AppData { .. }): State<AppData>,
    Path(guild_id): Path<serenity::all::GuildId>,
) -> Json<Vec<Arc<crate::pages::Page>>> {
    let Some(pages) = crate::pages::get_all_pages(guild_id).await else {
        return Json(vec![]);
    };

    Json(pages)
}

/// Executes an operation on a setting [SettingsOperation]
pub(crate) async fn settings_operation(
    State(AppData {
        serenity_context,
        data,
    }): State<AppData>,
    Path((guild_id, user_id)): Path<(serenity::all::GuildId, serenity::all::UserId)>,
    Json(req): Json<SettingsOperationRequest>,
) -> Json<CanonicalSettingsResult> {
    let op: OperationType = req.op;

    // Find the setting
    let Some(page) = crate::pages::get_page_by_id(guild_id, &req.template).await else {
        return Json(CanonicalSettingsResult::Err {
            error: "Template not found".to_string(),
        });
    };

    let mut setting = None;
    for setting_obj in page.settings.iter() {
        if setting_obj.id == req.setting_id {
            setting = Some(setting_obj);
            break;
        }
    }

    let Some(setting) = setting else {
        return Json(CanonicalSettingsResult::Err {
            error: "Setting not found".to_string(),
        });
    };

    match op {
        OperationType::View => {
            match ar_settings::cfg::settings_view(
                setting,
                &crate::pages::SettingExecutionData::new(data.clone(), serenity_context, user_id),
                req.fields,
            )
            .await
            {
                Ok(res) => Json(CanonicalSettingsResult::Ok { fields: res }),
                Err(e) => Json(CanonicalSettingsResult::Err {
                    error: e.to_string(),
                }),
            }
        }
        OperationType::Create => {
            match ar_settings::cfg::settings_create(
                setting,
                &crate::pages::SettingExecutionData::new(data.clone(), serenity_context, user_id),
                req.fields,
            )
            .await
            {
                Ok(res) => Json(CanonicalSettingsResult::Ok { fields: vec![res] }),
                Err(e) => Json(CanonicalSettingsResult::Err {
                    error: e.to_string(),
                }),
            }
        }
        OperationType::Update => {
            match ar_settings::cfg::settings_update(
                setting,
                &crate::pages::SettingExecutionData::new(data.clone(), serenity_context, user_id),
                req.fields,
            )
            .await
            {
                Ok(res) => Json(CanonicalSettingsResult::Ok { fields: vec![res] }),
                Err(e) => Json(CanonicalSettingsResult::Err {
                    error: e.to_string(),
                }),
            }
        }
        OperationType::Delete => {
            let Some(pkey) = req.fields.get(&setting.primary_key) else {
                return Json(CanonicalSettingsResult::Err {
                    error: format!("Missing or invalid field: `{}`", setting.primary_key),
                });
            };

            match ar_settings::cfg::settings_delete(
                setting,
                &crate::pages::SettingExecutionData::new(data.clone(), serenity_context, user_id),
                pkey.clone(),
            )
            .await
            {
                Ok(_res) => Json(CanonicalSettingsResult::Ok { fields: vec![] }),
                Err(e) => Json(CanonicalSettingsResult::Err {
                    error: e.to_string(),
                }),
            }
        }
    }
}
