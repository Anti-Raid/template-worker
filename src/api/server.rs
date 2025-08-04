use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use utoipa_axum::{router::OpenApiRouter, routes};
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
    let (internal_router, internal_openapi) = OpenApiRouter::new()
        .routes(
            routes!(
                internal_api::dispatch_event,
                internal_api::dispatch_event_and_wait,
                internal_api::regenerate_cache_api,
                internal_api::get_threads_count,
                internal_api::ping_all_threads,
                internal_api::clear_inactive_guilds,
                internal_api::remove_unused_threads,
                internal_api::close_thread,
                internal_api::execute_lua_vm_action,
                internal_api::get_vm_metrics_by_tid,
                internal_api::get_vm_metrics_for_all,
                internal_api::guilds_exist,
            )
        )
        .with_state::<AppData>(AppData::new(data.clone(), ctx))
        .split_for_parts();

    let (public_router, public_openapi) = OpenApiRouter::new()
            .route("/healthcheck", post(|| async { Json(()) }))
            .routes(
                routes!(
                    public_api::get_settings_for_guild_user,
                    public_api::execute_setting_for_guild_user,
                    public_api::get_user_guilds,
                    public_api::base_guild_user_info,
                    public_api::create_oauth2_session,
                    public_api::get_authorized_session,
                    public_api::get_user_sessions_api,
                    public_api::create_user_session,
                    public_api::delete_user_session_api,
                    public_api::state,
                    public_api::api_config,
                    public_api::get_bot_stats
                )
            )
            .with_state::<AppData>(AppData::new(data.clone(), ctx))
            .split_for_parts();

    let router = Router::new()
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .merge(internal_router)
        .merge(public_router)
        .route("/i/openapi", get(|| async { Json(internal_openapi) }))
        .route("/openapi", get(|| async { Json(public_openapi) }))
        .fallback(
            get(|| async { (StatusCode::NOT_FOUND, Json(ApiError::from("Not Found"))) })
        );

    let router: Router<()> = router.with_state(AppData::new(data, ctx));
    router.into_make_service()
}