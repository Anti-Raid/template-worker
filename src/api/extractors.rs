use super::server::{ApiError, ApiErrorCode, ApiResponseError, AppData};
use axum::extract::FromRequestParts;
use axum::Json;

/// This extractor checks if the user is authorized
/// from the DB and if so, returns the user id
pub struct AuthorizedUser {
    pub user_id: String,
    pub session_id: String,
    pub state: String,
    pub session_type: String,
}

impl FromRequestParts<AppData> for AuthorizedUser {
    type Rejection = ApiResponseError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppData,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                (
                    axum::http::StatusCode::UNAUTHORIZED,
                    Json(ApiError {
                        message: "Whoa there! This endpoint requires authentication to use!"
                            .to_string(),
                        code: ApiErrorCode::NoAuthToken,
                    }),
                )
            })?;

        let auth_response = crate::api::auth::check_web_auth(&state.data.pool, token)
            .await
            .map_err(|e| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError {
                        message: format!("Failed to check auth for token due to error: {e:?}"),
                        code: ApiErrorCode::InternalAuthError,
                    }),
                )
            })?;

        match auth_response {
            crate::api::auth::AuthResponse::Success {
                user_id,
                session_id,
                state,
                session_type,
            } => Ok(AuthorizedUser {
                user_id,
                session_id,
                session_type,
                state,
            }),
            crate::api::auth::AuthResponse::ApiBanned { user_id, .. } => {
                return Err((
                    axum::http::StatusCode::FORBIDDEN,
                    Json(ApiError {
                        message: format!("User {} is banned from using the API", user_id),
                        code: ApiErrorCode::ApiBanned,
                    }),
                ));
            }
            crate::api::auth::AuthResponse::InvalidToken => Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    message:
                        "The token provided is invalid. Check that it hasn't expired and try again?"
                            .to_string(),
                    code: ApiErrorCode::InvalidToken,
                }),
            )),
        }
    }
}

/// This extractor checks if the user is authorized
/// and is able to access internal endpoints
#[allow(dead_code)]
pub struct InternalEndpoint {
    pub user_id: String,
}

impl FromRequestParts<AppData> for InternalEndpoint {
    type Rejection = ApiResponseError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &AppData,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                (
                    axum::http::StatusCode::UNAUTHORIZED,
                    Json(ApiError {
                        message: "Whoa there! This endpoint requires authentication to use!"
                            .to_string(),
                        code: ApiErrorCode::NoAuthToken,
                    }),
                )
            })?;

        let auth_response = crate::api::auth::check_web_auth(&state.data.pool, token)
            .await
            .map_err(|e| {
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ApiError {
                        message: format!("Failed to check auth for token due to error: {e:?}"),
                        code: ApiErrorCode::InternalAuthError,
                    }),
                )
            })?;

        match auth_response {
            crate::api::auth::AuthResponse::Success { user_id, .. } => {
                let user_id_discord: serenity::all::UserId =
                    user_id.parse().map_err(|e: serenity::all::ParseIdError| {
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ApiError {
                                message: format!("Failed to parse user ID: {}", e),
                                code: ApiErrorCode::InternalError,
                            }),
                        )
                    })?;

                if !crate::CONFIG
                    .discord_auth
                    .root_users
                    .contains(&user_id_discord)
                {
                    return Err((
                        axum::http::StatusCode::FORBIDDEN,
                        Json(ApiError {
                            message: "This endpoint is restricted to only AntiRaid staff members/root members".to_string(),
                            code: ApiErrorCode::Restricted,
                        }),
                    ));
                }

                Ok(InternalEndpoint { user_id })
            }
            crate::api::auth::AuthResponse::ApiBanned { user_id, .. } => {
                return Err((
                    axum::http::StatusCode::FORBIDDEN,
                    Json(ApiError {
                        message: format!("User {} is banned from using the API", user_id),
                        code: ApiErrorCode::ApiBanned,
                    }),
                ));
            }
            crate::api::auth::AuthResponse::InvalidToken => Err((
                axum::http::StatusCode::UNAUTHORIZED,
                Json(ApiError {
                    message:
                        "The token provided is invalid. Check that it hasn't expired and try again?"
                            .to_string(),
                    code: ApiErrorCode::InvalidToken,
                }),
            )),
        }
    }
}
