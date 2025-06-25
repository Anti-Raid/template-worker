use crate::coresettings::data::RequestScope;
use crate::coresettings::data::SettingsData;
use crate::templatingrt::primitives::sandwich_config;
use crate::templatingrt::CreateGuildState;
use crate::templatingrt::POOL;
use crate::templatingrt::{cache::regenerate_cache, MAX_TEMPLATES_RETURN_WAIT_TIME};
use crate::vmbench::{benchmark_vm as benchmark_vm_impl, FireBenchmark};
use antiraid_types::ar_event::AntiraidEvent;
use ar_settings::types::OperationType;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::all::GuildChannel;
use serenity::all::Permissions;
use serenity::all::Role;
use serenity::all::RoleId;
use std::{collections::HashMap, sync::Arc};

use crate::dispatch::{dispatch, dispatch_and_wait, parse_event};
use crate::templatingrt::execute;

pub static STATE_CACHE: std::sync::LazyLock<Arc<TwState>> = std::sync::LazyLock::new(|| {
    let mut state = TwState {
        settings: Vec::with_capacity(crate::coresettings::config_options().len()),
    };

    for setting in crate::coresettings::config_options() {
        state.settings.push(setting);
    }

    Arc::new(state)
});

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
        .route("/healthcheck", post(|| async { Json(()) }))
        .route("/benchmark-vm/:guild_id", post(benchmark_vm))
        .route("/pages/:guild_id", post(get_pages_for_guild))
        .route(
            "/page-settings-operation/:guild_id/:user_id",
            post(page_settings_operation),
        )
        .route("/threads-count", get(get_threads_count))
        .route("/clear-inactive-guilds", post(clear_inactive_guilds))
        .route(
            "/execute-luavmaction/:guild_id",
            post(execute_lua_vm_action),
        )
        .route("/get-vm-metrics-by-tid/:tid", get(get_vm_metrics_by_tid))
        .route("/get-vm-metrics-for-all", get(get_vm_metrics_for_all))
        // Given a list of guild ids, return a set of 0s and 1s indicating whether each guild exists in cache [GuildsExist]
        .route("/guilds-exist", get(guilds_exist))
        // Returns basic user/guild information [BaseGuildUserInfo]
        .route(
            "/base-guild-user-info/:guild_id/:user_id",
            get(base_guild_user_info),
        )
        // Executes an operation on a setting [SettingsOperation]
        .route(
            "/settings-operation/:guild_id/:user_id",
            post(settings_operation),
        )
        // Executes an operation on a setting [SettingsOperationAnonymous]
        .route(
            "/settings-operation-anonymous",
            post(settings_operation_anonymous),
        )
        // Returns the bots state [BotState]
        .route("/state", get(state));
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
        regenerate_cache(&serenity_context, &data, guild_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
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
) -> Response<HashMap<String, serde_json::Value>> {
    // Regenerate cache for guild if event is OnStartup
    if let AntiraidEvent::OnStartup(_) = event {
        regenerate_cache(&serenity_context, &data, guild_id)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
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

/// Returns the number of threads running
async fn get_threads_count(State(AppData { .. }): State<AppData>) -> Response<usize> {
    let Ok(count) = POOL.len() else {
        return Ok(Json(0));
    };

    Ok(Json(count))
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
        CreateGuildState {
            pool: data.pool.clone(),
            serenity_context,
            reqwest_client: data.reqwest.clone(),
            object_store: data.object_store.clone(),
        },
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(bvm))
}

/// Flush out inactive guilds
async fn clear_inactive_guilds(
    State(AppData { .. }): State<AppData>,
) -> Response<Vec<crate::templatingrt::ThreadClearInactiveGuilds>> {
    let Ok(hm) = crate::templatingrt::POOL.clear_inactive_guilds().await else {
        return Err((
            reqwest::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to start inactive guild clear".to_string(),
        ));
    };

    Ok(Json(hm))
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ExecuteLuaVmActionOpts {
    pub wait_timeout: Option<std::time::Duration>,
}

/// Execute a lua vm action on a guild
#[axum::debug_handler]
async fn execute_lua_vm_action(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    Path(guild_id): Path<serenity::all::GuildId>,
    Query(opts): Query<ExecuteLuaVmActionOpts>,
    Json(action): Json<crate::templatingrt::LuaVmAction>,
) -> Response<crate::templatingrt::MultiLuaVmResultHandle> {
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
            e.to_string().into(),
        )
    })?;

    let result_handle = match handle
        .wait_timeout(opts.wait_timeout.unwrap_or(MAX_TEMPLATES_RETURN_WAIT_TIME))
        .await
    {
        Ok(Some(action)) => action,
        Ok(None) => {
            return Err((
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                "Timed out while waiting for response".into(),
            ))
        }
        Err(e) => {
            return Err((
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string().into(),
            ))
        }
    };

    Ok(Json(result_handle))
}

/// Get thread pool metrics given tid
#[axum::debug_handler]
async fn get_vm_metrics_by_tid(
    Path(tid): Path<u64>,
) -> Response<Vec<crate::templatingrt::ThreadMetrics>> {
    let metrics = crate::templatingrt::POOL
        .get_vm_metrics_by_tid(tid)
        .await
        .map_err(|e| {
            (
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string().into(),
            )
        })?;

    Ok(Json(metrics))
}

/// Get thread pool metrics given tid
#[axum::debug_handler]
async fn get_vm_metrics_for_all() -> Response<Vec<crate::templatingrt::ThreadMetrics>> {
    let metrics = crate::templatingrt::POOL
        .get_vm_metrics_for_all()
        .await
        .map_err(|e| {
            (
                reqwest::StatusCode::INTERNAL_SERVER_ERROR,
                e.to_string().into(),
            )
        })?;

    Ok(Json(metrics))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSettingsOperationRequest {
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
pub(crate) async fn page_settings_operation(
    State(AppData {
        serenity_context,
        data,
    }): State<AppData>,
    Path((guild_id, user_id)): Path<(serenity::all::GuildId, serenity::all::UserId)>,
    Json(req): Json<PageSettingsOperationRequest>,
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
            match ar_settings::cfg::settings_delete(
                setting,
                &crate::pages::SettingExecutionData::new(data.clone(), serenity_context, user_id),
                req.fields,
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

/// Given a list of guild ids, return a set of 0s and 1s indicating whether each guild exists in cache [GuildsExist]
#[axum::debug_handler]
pub(crate) async fn guilds_exist(
    State(AppData {
        data,
        serenity_context,
    }): State<AppData>,
    Json(guilds): Json<Vec<serenity::all::GuildId>>,
) -> Response<Vec<i32>> {
    let mut guilds_exist = Vec::with_capacity(guilds.len());

    for guild in guilds {
        let has_guild = sandwich_driver::has_guild(
            &serenity_context.cache,
            &serenity_context.http,
            &data.reqwest,
            guild,
            &sandwich_config(),
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        guilds_exist.push({
            if has_guild {
                1
            } else {
                0
            }
        });
    }

    Ok(Json(guilds_exist))
}

/// Returns basic user/guild information [BaseGuildUserInfo]
async fn base_guild_user_info(
    State(AppData {
        data,
        serenity_context,
        ..
    }): State<AppData>,
    Path((guild_id, user_id)): Path<(serenity::all::GuildId, serenity::all::UserId)>,
) -> Response<BaseGuildUserInfo> {
    let bot_user_id = serenity_context.cache.current_user().id;
    let guild = sandwich_driver::guild(
        &serenity_context.cache,
        &serenity_context.http,
        &data.reqwest,
        guild_id,
        &sandwich_config(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get guild: {:#?}", e),
        )
    })?;

    // Next fetch the member and bot_user
    let member: serenity::model::prelude::Member = match sandwich_driver::member_in_guild(
        &serenity_context.cache,
        &serenity_context.http,
        &data.reqwest,
        guild_id,
        user_id,
        &sandwich_config(),
    )
    .await
    {
        Ok(Some(member)) => member,
        Ok(None) => {
            return Err((StatusCode::NOT_FOUND, "User not found".into()));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get member: {:#?}", e),
            ));
        }
    };

    let bot_user: serenity::model::prelude::Member = match sandwich_driver::member_in_guild(
        &serenity_context.cache,
        &serenity_context.http,
        &data.reqwest,
        guild_id,
        bot_user_id,
        &sandwich_config(),
    )
    .await
    {
        Ok(Some(member)) => member,
        Ok(None) => {
            return Err((StatusCode::NOT_FOUND, "Bot user not found".into()));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to get bot user: {:#?}", e),
            ));
        }
    };

    // Fetch the channels
    let channels = sandwich_driver::guild_channels(
        &serenity_context.cache,
        &serenity_context.http,
        &data.reqwest,
        guild_id,
        &sandwich_config(),
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to get channels: {:#?}", e),
        )
    })?;

    let mut channels_with_permissions = Vec::with_capacity(channels.len());

    for channel in channels.iter() {
        channels_with_permissions.push(GuildChannelWithPermissions {
            user: guild.user_permissions_in(channel, &member),
            bot: guild.user_permissions_in(channel, &bot_user),
            channel: channel.clone(),
        });
    }

    Ok(Json(BaseGuildUserInfo {
        name: guild.name.to_string(),
        icon: guild.icon_url(),
        owner_id: guild.owner_id.to_string(),
        roles: guild.roles.into_iter().collect(),
        user_roles: member.roles.to_vec(),
        bot_roles: bot_user.roles.to_vec(),
        channels: channels_with_permissions,
    }))
}

// Temporary OPS below

/// Executes an operation on a setting [SettingsOperation]
pub(crate) async fn settings_operation(
    State(AppData {
        serenity_context, ..
    }): State<AppData>,
    Path((guild_id, user_id)): Path<(serenity::all::GuildId, serenity::all::UserId)>,
    Json(req): Json<SettingsOperationRequest>,
) -> Json<CanonicalSettingsResult> {
    let op: OperationType = req.op;

    // Find the setting
    let mut setting = None;

    for setting_obj in crate::coresettings::config_options() {
        if setting_obj.id == req.setting {
            setting = Some(setting_obj);
            break;
        }
    }

    //if let Some(page_setting) = templating::cache::get_setting(guild_id, &req.setting).await {
    //    setting = Some(page_setting);
    //};

    let Some(setting) = setting else {
        return Json(CanonicalSettingsResult::Err {
            error: "Setting not found".to_string(),
        });
    };

    match op {
        OperationType::View => {
            match ar_settings::cfg::settings_view(
                &setting,
                &crate::coresettings::data::settings_data(
                    serenity_context,
                    RequestScope::Guild((guild_id, user_id)),
                ),
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
                &setting,
                &crate::coresettings::data::settings_data(
                    serenity_context,
                    RequestScope::Guild((guild_id, user_id)),
                ),
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
                &setting,
                &crate::coresettings::data::settings_data(
                    serenity_context,
                    RequestScope::Guild((guild_id, user_id)),
                ),
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
            match ar_settings::cfg::settings_delete(
                &setting,
                &crate::coresettings::data::settings_data(
                    serenity_context,
                    RequestScope::Guild((guild_id, user_id)),
                ),
                req.fields,
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

/// Executes an operation on a setting [SettingsOperationAnonymous]
pub(crate) async fn settings_operation_anonymous(
    State(AppData {
        serenity_context, ..
    }): State<AppData>,
    Json(req): Json<SettingsOperationRequest>,
) -> Json<CanonicalSettingsResult> {
    let op: OperationType = req.op;

    // Find the setting
    let mut setting = None;

    for setting_obj in crate::coresettings::config_options() {
        if setting_obj.id == req.setting {
            setting = Some(setting_obj);
            break;
        }
    }

    //if let Some(page_setting) = templating::cache::get_setting(guild_id, &req.setting).await {
    //    setting = Some(page_setting);
    //};

    let Some(setting) = setting else {
        return Json(CanonicalSettingsResult::Err {
            error: "Setting not found".to_string(),
        });
    };

    match op {
        OperationType::View => {
            match ar_settings::cfg::settings_view(
                &setting,
                &crate::coresettings::data::settings_data(
                    serenity_context,
                    RequestScope::Anonymous,
                ),
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
                &setting,
                &crate::coresettings::data::settings_data(
                    serenity_context,
                    RequestScope::Anonymous,
                ),
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
                &setting,
                &crate::coresettings::data::settings_data(
                    serenity_context,
                    RequestScope::Anonymous,
                ),
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
            match ar_settings::cfg::settings_delete(
                &setting,
                &crate::coresettings::data::settings_data(
                    serenity_context,
                    RequestScope::Anonymous,
                ),
                req.fields,
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

// End of temporary API's

/// Returns a list of modules [Modules]
async fn state(State(AppData { .. }): State<AppData>) -> Json<Arc<TwState>> {
    Json(STATE_CACHE.clone())
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GuildChannelWithPermissions {
    pub user: Permissions,
    pub bot: Permissions,
    pub channel: GuildChannel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BaseGuildUserInfo {
    pub owner_id: String,
    pub name: String,
    pub icon: Option<String>,
    /// List of all roles in the server
    pub roles: Vec<Role>,
    /// List of roles the user has
    pub user_roles: Vec<RoleId>,
    /// List of roles the bot has
    pub bot_roles: Vec<RoleId>,
    /// List of all channels in the server
    pub channels: Vec<GuildChannelWithPermissions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsOperationRequest {
    pub fields: indexmap::IndexMap<String, Value>,
    pub op: ar_settings::types::OperationType,
    pub setting: String,
}

#[derive(Serialize, Deserialize)]
pub struct TwState {
    pub settings: Vec<ar_settings::types::Setting<SettingsData>>,
}
