use axum::{
    http::StatusCode,
    routing::{get, post, delete},
    Json, Router,
};
use std::sync::Arc;
use super::internal_api;
use super::public_api;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub enum ApiErrorCode {
    InternalAuthError,
    NoAuthToken,
    ApiBanned,
    InvalidToken,
    InternalError,
    Restricted
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ApiError {
    pub message: String,
    pub code: ApiErrorCode
}

impl From<String> for ApiError {
    fn from(message: String) -> Self {
        ApiError {
            message,
            code: ApiErrorCode::InternalError,
        }
    }
}

impl<'a> From<&'a str> for ApiError {
    fn from(message: &'a str) -> Self {
        ApiError {
            message: message.to_string(),
            code: ApiErrorCode::InternalError,
        }
    }
}

#[derive(Clone)]
pub struct AppData {
    pub data: Arc<crate::data::Data>,
    pub serenity_context: serenity::all::Context,
}

impl AppData {
    pub fn new(data: Arc<crate::data::Data>, ctx: &serenity::all::Context) -> Self {
        Self {
            data,
            serenity_context: ctx.clone(),
        }
    }
}

pub type ApiResponseError = (StatusCode, Json<ApiError>);
pub type ApiResponse<T> = Result<Json<T>, ApiResponseError>;


pub fn create(
    data: Arc<crate::data::Data>,
    ctx: &serenity::all::Context,
) -> axum::routing::IntoMakeService<Router> {
    let router = Router::new()
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .route("/healthcheck", post(|| async { Json(()) }))
        //@ Settings
        .route(
            "/guilds/{guild_id}/settings",
            get(public_api::get_settings_for_guild_user),
        )
        .route(
            "/guilds/{guild_id}/settings",
            post(public_api::execute_setting_for_guild_user),
        )
        //@ User Base
        .route(
            "/users/@me/guilds",
            get(public_api::get_user_guilds),
        )
        // Returns basic user/guild information
        .route(
            "/users/@me/guilds/{guild_id}",
            get(public_api::base_guild_user_info),
        )
        //@ Auth
        // Creates a login token from a Discord OAuth2 login [CreateUserSession]
        .route("/oauth2", post(public_api::create_oauth2_session))
        // Returns info about the current session of the user and the user's info itself
        .route("/sessions/@me", get(public_api::get_authorized_session)) // NOTE: Replaces webapi's /test-auth
        // Gets the list of sessions the user has on AntiRaid
        .route("/sessions", get(public_api::get_user_sessions_api))
        // Creates a new user session
        .route("/sessions", post(public_api::create_user_session))
        // Deletes a user session
        .route("/sessions/{session_id}", delete(public_api::delete_user_session_api))
        //@ Core
        // Returns the API configuration
        .route("/config", get(public_api::api_config))
        // Returns the bots state [BotState]
        .route("/bot-state", get(public_api::state))
        .route("/bot-stats", get(public_api::get_bot_stats))
        // TODO: template-shop and template-shop/{name} will wait till shop is rereleased/open

        // Internal API routes (designated by /i/)
        
        .route("/i/dispatch-event/{guild_id}", post(internal_api::dispatch_event))
        .route(
            "/i/dispatch-event/{guild_id}/@wait",
            post(internal_api::dispatch_event_and_wait),
        )
        .route(
            "/i/regenerate-cache/{guild_id}",
            post(internal_api::regenerate_cache_api),
        )
        .route("/i/ping-all-threads", post(internal_api::ping_all_threads))
        .route("/i/threads-count", get(internal_api::get_threads_count))
        .route("/i/clear-inactive-guilds", post(internal_api::clear_inactive_guilds))
        .route("/i/remove_unused_threads", post(internal_api::remove_unused_threads))
        .route("/i/close-thread/{tid}", post(internal_api::close_thread))
        .route(
            "/i/execute-luavmaction/{guild_id}",
            post(internal_api::execute_lua_vm_action),
        )
        .route("/i/get-vm-metrics-by-tid/{tid}", get(internal_api::get_vm_metrics_by_tid))
        .route("/i/get-vm-metrics-for-all", get(internal_api::get_vm_metrics_for_all))
        // Given a list of guild ids, return a set of 0s and 1s indicating whether each guild exists in cache [GuildsExist]
        .route("/i/guilds-exist", get(internal_api::guilds_exist));
    let router: Router<()> = router.with_state(AppData::new(data, ctx));
    router.into_make_service()
}