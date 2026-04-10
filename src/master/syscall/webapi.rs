use axum::response::Response;
use axum::{extract::FromRequestParts, response::IntoResponse};
use axum::Json;
use reqwest::StatusCode;
use serenity::all::UserId;

use crate::master::syscall::MSyscallRet;
use crate::master::syscall::{MSyscallError, MSyscallHandler, internal::auth as iauth};

impl IntoResponse for MSyscallRet {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

impl IntoResponse for MSyscallError {
    fn into_response(self) -> Response {
        (StatusCode::BAD_REQUEST, Json(self)).into_response()
    }
}

/// This extractor checks if the user is authorized
/// from the DB and if so, returns the user id
struct AuthorizedUser {
    pub user_id: UserId,
    pub session_id: String,
    pub state: String,
    pub session_type: String,
}

impl FromRequestParts<MSyscallHandler> for AuthorizedUser {
    type Rejection = MSyscallError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &MSyscallHandler,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| MSyscallError::Unauthorized { reason: "No Authorization header found" })?;

        let auth_response = iauth::check_web_auth(&state.pool, token)
            .await?;

        match auth_response {
            iauth::AuthResponse::Success {
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
            iauth::AuthResponse::ApiBanned { user_id, .. } => {
                return Err(MSyscallError::Unauthorized { reason: "You have banned from using this service" })
            }
            iauth::AuthResponse::InvalidToken => return Err(MSyscallError::Unauthorized { reason: "The token provided is invalid. Check that it hasn't expired and try again?" })
        }
    }
}