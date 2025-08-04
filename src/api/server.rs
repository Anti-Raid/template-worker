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
    Restricted,
    NotFound
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
        .routes(routes!(internal_api::dispatch_event))
        .routes(routes!(internal_api::dispatch_event_and_wait))
        .routes(routes!(internal_api::regenerate_cache_api))
        .routes(routes!(internal_api::get_threads_count))
        .routes(routes!(internal_api::ping_all_threads))
        .routes(routes!(internal_api::clear_inactive_guilds))
        .routes(routes!(internal_api::remove_unused_threads))
        .routes(routes!(internal_api::close_thread))
        .routes(routes!(internal_api::execute_lua_vm_action))
        .routes(routes!(internal_api::get_vm_metrics_by_tid))
        .routes(routes!(internal_api::get_vm_metrics_for_all))
        .routes(routes!(internal_api::guilds_exist))
        .with_state::<AppData>(AppData::new(data.clone(), ctx))
        .split_for_parts();

    let (public_router, public_openapi) = OpenApiRouter::new()
        .routes(routes!(public_api::get_settings_for_guild_user))
        .routes(routes!(public_api::execute_setting_for_guild_user))
        .routes(routes!(public_api::get_user_guilds))
        .routes(routes!(public_api::base_guild_user_info))
        .routes(routes!(public_api::create_oauth2_session))
        .routes(routes!(public_api::get_authorized_session))
        .routes(routes!(public_api::get_user_sessions_api))
        .routes(routes!(public_api::create_user_session))
        .routes(routes!(public_api::delete_user_session_api))
        .routes(routes!(public_api::state))
        .routes(routes!(public_api::api_config))
        .routes(routes!(public_api::get_bot_stats))
        .with_state::<AppData>(AppData::new(data.clone(), ctx))
        .split_for_parts();

    let router = Router::new()
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .merge(internal_router)
        .merge(public_router)
        .route("/healthcheck", post(|| async { Json(()) }))
        .route("/i/openapi", get(|| async { Json(internal_openapi) }))
        .route("/openapi", get(|| async { Json(public_openapi) }))
        .fallback(
            get(|| async { (StatusCode::NOT_FOUND, Json(ApiError {
                message: "Not Found".to_string(),
                code: ApiErrorCode::NotFound,
            })) })
        );

    let router: Router<()> = router.with_state(AppData::new(data, ctx));
    router.into_make_service()
}