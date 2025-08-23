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
        log::error!("Returning error: {message}");
        ApiError {
            message: message.to_string(),
            code: ApiErrorCode::InternalError,
        }
    }
}

#[derive(Clone)]
pub struct AppData {
    pub data: Arc<crate::data::Data>,
    pub http: Arc<serenity::http::Http>,
}

impl AppData {
    pub fn new(data: Arc<crate::data::Data>, http: Arc<serenity::http::Http>,) -> Self {
        Self {
            data,
            http
        }
    }
}

pub type ApiResponseError = (StatusCode, Json<ApiError>);
pub type ApiResponse<T> = Result<Json<T>, ApiResponseError>;

async fn logger(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    log::info!("Received request: method = {}, path={}", request.method(), request.uri().path());

    let response = next.run(request).await;
    response
}

pub fn create(data: Arc<crate::data::Data>, http: Arc<serenity::http::Http>) -> axum::routing::IntoMakeService<Router> {
    let mut router = Router::new();
    
    // Internal routes
    let internal_routes = [
        routes!(internal_api::dispatch_event),
        routes!(internal_api::dispatch_event_and_wait),
        routes!(internal_api::regenerate_cache_api),
        routes!(internal_api::get_threads_count),
        //routes!(internal_api::get_vm_metrics_by_tid),
        //routes!(internal_api::get_vm_metrics_for_all),
        routes!(internal_api::guilds_exist),
        routes!(internal_api::kill_worker)
    ];

    // Public routes
    let public_routes = [
        routes!(public_api::get_settings_for_guild_user),
        routes!(public_api::execute_setting_for_guild_user),
        routes!(public_api::get_user_guilds),
        routes!(public_api::base_guild_user_info),
        routes!(public_api::create_oauth2_session),
        routes!(public_api::get_authorized_session),
        routes!(public_api::get_user_sessions_api),
        routes!(public_api::create_user_session),
        routes!(public_api::delete_user_session_api),
        routes!(public_api::state),
        routes!(public_api::api_config),
        routes!(public_api::get_bot_stats),
    ];

    let mut oapi_router = OpenApiRouter::new();
    for route in internal_routes {
        oapi_router = oapi_router.routes(route.clone());

        let mut paths = route.1.paths;
        let path = {
            assert!(paths.len() == 1, "Internal API routes should have one path");
            let first_entry = paths.first_entry();
            let path = first_entry.map(|path| path.key().clone()).unwrap();
            if path.is_empty() { "/".to_string() } else { path }
        };

        router = router.route(&path, route.2);
    }

    let mut internal_openapi = oapi_router.into_openapi();

    // Add InternalAuth
    if let Some(comps) = internal_openapi.components.as_mut() {
        comps.security_schemes.insert(
            "InternalAuth".to_string(),
            SecurityScheme::ApiKey(ApiKey::Header(
                ApiKeyValue::with_description("Authorization", "API token. Note that user must have root access to use this API")
            )),
        );
    }

    let mut oapi_router = OpenApiRouter::new();
    for route in public_routes {
        oapi_router = oapi_router.routes(route.clone());

        let mut paths = route.1.paths;
        let path = {
            assert!(paths.len() == 1, "Public API routes should have one path");
            let first_entry = paths.first_entry();
            let path = first_entry.map(|path| path.key().clone()).unwrap();
            if path.is_empty() { "/".to_string() } else { path }
        };

        router = router.route(&path, route.2);
    }

    let mut public_openapi = oapi_router.into_openapi();

    // Add PublicAuth
    if let Some(comps) = public_openapi.components.as_mut() {
        comps.security_schemes.insert(
            "PublicAuth".to_string(),
            SecurityScheme::ApiKey(ApiKey::Header(
                ApiKeyValue::with_description("Authorization", "API token. This API is public but requires authentication")
            )),
        );
    }

    router = router
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
            axum::middleware::from_fn(logger)
        );

    let router: Router<()> = router.with_state(AppData::new(data, http));
    router.into_make_service()
}
