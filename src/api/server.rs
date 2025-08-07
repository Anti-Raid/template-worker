use axum::{
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa_axum::{router::OpenApiRouter, routes};
use utoipa_swagger_ui::SwaggerUi;
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
    NotFound,
    BadRequest
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
    let (internal_router, mut internal_openapi) = OpenApiRouter::new()
        .routes(routes!(internal_api::dispatch_event))
        .routes(routes!(internal_api::dispatch_event_and_wait))
        .routes(routes!(internal_api::regenerate_cache_api))
        .routes(routes!(internal_api::get_threads_count))
        //.routes(routes!(internal_api::get_vm_metrics_by_tid))
        //.routes(routes!(internal_api::get_vm_metrics_for_all))
        .routes(routes!(internal_api::guilds_exist))
        .with_state::<AppData>(AppData::new(data.clone(), ctx))
        .split_for_parts();

    // Add InternalAuth
    if let Some(comps) = internal_openapi.components.as_mut() {
        comps.security_schemes.insert(
            "InternalAuth".to_string(),
            SecurityScheme::ApiKey(ApiKey::Header(
                ApiKeyValue::with_description("Authorization", "API token. Note that user must have root access to use this API")
            )),
        );
    }

    let (public_router, mut public_openapi) = OpenApiRouter::new()
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

    // Add PublicAuth
    if let Some(comps) = public_openapi.components.as_mut() {
        comps.security_schemes.insert(
            "PublicAuth".to_string(),
            SecurityScheme::ApiKey(ApiKey::Header(
                ApiKeyValue::with_description("Authorization", "API token. This API is public but requires authentication")
            )),
        );
    }

    let router = Router::new()
        .merge(internal_router)
        .merge(public_router)
        .route("/healthcheck", post(|| async { Json(()) }))
        .merge(
            SwaggerUi::new("/docs")
            .url("/openapi", public_openapi)
        )
        .merge(SwaggerUi::new("/i/docs").url("/i/openapi", internal_openapi))
        .fallback(
            get(|| async { (StatusCode::NOT_FOUND, Json(ApiError {
                message: "Not Found".to_string(),
                code: ApiErrorCode::NotFound,
            })) })
        )
        .layer(tower_http::cors::CorsLayer::very_permissive())
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
            .make_span_with(|req: &axum::http::Request<_>| {
                tracing::info_span!(
                    "http_request",
                    method = %req.method(),
                    uri = %req.uri(),
                    version = ?req.version(),
                )
            })
        );

    let router: Router<()> = router.with_state(AppData::new(data, ctx));
    router.into_make_service()
}