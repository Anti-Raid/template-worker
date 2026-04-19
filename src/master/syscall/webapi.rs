use axum::response::Response;
use axum::routing::{get, post};
use axum::{extract::{State, FromRequestParts, Json}, Router, response::IntoResponse};
use reqwest::StatusCode;
use reqwest::header::AUTHORIZATION;
use serenity::all::UserId;
use crate::master::syscall::bot::MBotSyscall;
use crate::master::syscall::{MSyscallArgs, MSyscallContext, MSyscallRet};
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
    pub session_type: String
}

struct OptionalAuthorizedUser(Option<AuthorizedUser>);

impl FromRequestParts<MSyscallHandler> for OptionalAuthorizedUser {
    type Rejection = MSyscallError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &MSyscallHandler,
    ) -> Result<Self, Self::Rejection> {
        if parts.headers.contains_key(AUTHORIZATION) {
            Ok(Self(Some(AuthorizedUser::from_request_parts(parts, state).await?)))
        } else {
            Ok(Self(None))
        }
    }
}

impl FromRequestParts<MSyscallHandler> for AuthorizedUser {
    type Rejection = MSyscallError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &MSyscallHandler,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| MSyscallError::Unauthorized { reason: "No Authorization header found" })?;

        let auth_response = iauth::check_web_auth(&state.pool, token).await?;

        match auth_response {
            iauth::AuthResponse::Success { user_id, session_type, .. } => Ok(AuthorizedUser { user_id, session_type }),
            iauth::AuthResponse::ApiBanned { .. } => {
                return Err(MSyscallError::Unauthorized { reason: "You have banned from using this service" })
            }
            iauth::AuthResponse::InvalidToken => return Err(MSyscallError::Unauthorized { reason: "The token provided is invalid. Check that it hasn't expired and try again?" })
        }
    }
}

pub fn create(handler: MSyscallHandler) -> axum::routing::IntoMakeService<Router> {
    async fn logger(
        request: axum::extract::Request,
        next: axum::middleware::Next,
    ) -> axum::response::Response {
        log::info!(
            "Received request: method = {}, path={}",
            request.method(),
            request.uri().path()
        );

        let response = next.run(request).await;
        response
    }

    pub(super) async fn msyscall(
        user: OptionalAuthorizedUser,
        State(handler): State<MSyscallHandler>,
        Json(args): Json<MSyscallArgs>,
    ) -> Result<MSyscallRet, MSyscallError> {
        let ctx = if let Some(user) = user.0 { 
            match user.session_type.as_str() {
                "login" | "app_login" => MSyscallContext::ApiOauth(user.user_id),
                _ => MSyscallContext::ApiToken(user.user_id) 
            }
        } else { MSyscallContext::ApiAnon };
        let resp = handler.handle_syscall(args, ctx).await?;
        Ok(resp)
    }

    let mut router = Router::new();

    router = router
        .route("/healthcheck", post(|| async { Json(()) }))
        .route("/msyscall", post(msyscall))
        // GET apis for better caching etc.
        .route("/commands", get(async |State(handler): State<MSyscallHandler>| {
            handler.handle_syscall(MSyscallArgs::Bot { req: MBotSyscall::GetBotCommands {} }, MSyscallContext::ApiAnonGetter).await
        }))
        .route("/status", get(async |State(handler): State<MSyscallHandler>| {
            handler.handle_syscall(MSyscallArgs::Bot { req: MBotSyscall::GetBotStatus {} }, MSyscallContext::ApiAnonGetter).await
        }))
        .fallback(get(|| async {
            (
                StatusCode::NOT_FOUND,
                "Use /msyscall for msyscall (insecure) and /msyscall/secure for msyscall (secure, staff-only)"
            )
        }))
        .layer(tower_http::cors::CorsLayer::very_permissive())
        .layer(axum::middleware::from_fn(logger));

    let router: Router<()> = router.with_state(handler);
    router.into_make_service()
}