use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use axum::extract::WebSocketUpgrade;
use axum::extract::ws::{CloseFrame, Message, WebSocket};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{extract::{State, FromRequestParts, Json}, Router, response::IntoResponse};
use dapi::UserId;
use khronos_runtime::futures_util::{SinkExt, StreamExt};
use khronos_runtime::utils::khronos_value::CKhronosValue;
use reqwest::{StatusCode, header};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use tokio_stream::StreamMap;
use tokio_stream::wrappers::UnboundedReceiverStream;
use tower_http::cors::MaxAge;
use crate::geese::stream::StreamId;
use crate::master::syscall::bot::{MBotSyscall, MBotSyscallRet};
use crate::master::syscall::{MSyscallArgs, MSyscallContext, MSyscallRet};
use crate::master::syscall::{MSyscallError, MSyscallHandler, internal::auth as iauth};

impl IntoResponse for MSyscallRet {
    fn into_response(self) -> Response {
        (StatusCode::OK, Json(self)).into_response()
    }
}

impl IntoResponse for MSyscallError {
    fn into_response(self) -> Response {
        match self {
            MSyscallError::Ratelimited { retry_after, .. } => {
                (
                    StatusCode::TOO_MANY_REQUESTS, 
                    [
                        ("Retry-After", retry_after.to_string()),
                    ],
                    Json(self)
                ).into_response()
            },
            _ => (StatusCode::BAD_REQUEST, Json(self)).into_response()
        }
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

    async fn msyscall(
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

    #[derive(serde::Deserialize)]
    struct Presigned {
        #[serde(rename = "p")]
        payload: String,
        #[serde(rename = "s")]
        sig: String,
    }

    async fn get_presigned(
        State(handler): State<MSyscallHandler>,
        axum::extract::Query(p): axum::extract::Query<Presigned>
    ) -> impl IntoResponse {
        enum Resp {
            Err(MSyscallError),
            Data { data: Vec<u8>, filename: String }
        }
        impl IntoResponse for Resp {
            fn into_response(self) -> Response {
                match self {
                    Resp::Err(e) => (StatusCode::BAD_REQUEST, Json(e)).into_response(),
                    Resp::Data { data, filename } => {
                        let resp = Response::builder()
                            .header(header::CONTENT_TYPE, "application/octet-stream")
                            .header(header::CONTENT_DISPOSITION, format!("attachment; filename=\"{}\"", filename))
                            .header(header::CACHE_CONTROL, "public, max-age=31536000, immutable")
                            .header(header::CONTENT_LENGTH, data.len())
                            .body(axum::body::Body::from(data));

                        let Ok(resp) = resp else {
                            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to build response").into_response();
                        };  

                        resp.into_response()
                    }
                }
            }
        }
        match handler.handle_syscall(MSyscallArgs::Bot { req: MBotSyscall::GetBlobData { payload: p.payload, signature: p.sig } }, MSyscallContext::ApiAnonGetter).await {
            Ok(MSyscallRet::Bot { data: MBotSyscallRet::BlobData { data, filename } }) => Resp::Data { data, filename },
            Ok(_) => Resp::Err(MSyscallError::EntityNotFound { reason: "Failed to get blob back from server" }),
            Err(e) => Resp::Err(e)
        }
    }

    async fn ws(
        ws: WebSocketUpgrade,
        State(state): State<MSyscallHandler>,
    ) -> Response {
        #[derive(Serialize, Deserialize)]
        pub enum WsMessage {
            Sub { id: String }, // can be sent by either client or server
            DropSub { id: String }, // can be sent by either client or server
            Hb {}, // heartbeat, can be sent by either client or server
            Warn { msg: String }, // server only, will be ignored if sent by server rn
            Msg { id: String, msg: CKhronosValue } // can be sent by either client or server
        }

        impl WsMessage {
            fn to_msg(&self) -> Result<Message, crate::Error> {
                let s = serde_json::to_string(self)?;
                Ok(Message::Text(s.into()))
            }
        }

        async fn send_close_message(socket: &mut WebSocket, code: u16, reason: &str) {
            _ = socket.send(Message::Close(Some(CloseFrame {
                code: code,
                reason: reason.into(),
            })))
            .await;
        }

        async fn handle_socket(socket: &mut WebSocket, state: MSyscallHandler) -> Result<(), crate::Error> {
            let (mut ws_sender, mut ws_receiver) = socket.split();
            let mut multiplexed_receivers = StreamMap::new();
            let mut active_guards = HashMap::new();
            loop {
                tokio::select! {
                    msg = ws_receiver.next() => {
                        let Some(msg) = msg else { break };
                        let msg = match msg? {
                            Message::Text(b) => serde_json::from_slice::<WsMessage>(b.as_bytes())?,
                            _ => continue
                        };
                        match msg {
                            WsMessage::Sub { id } => {
                                let sid = StreamId::from_str(&id)?;
                                let Some((ag, rx)) = state.worker_pool.attach_stream(sid) else { 
                                    ws_sender.send(WsMessage::Warn { msg: "Cannot attach this stream (already attached?)".to_string() }.to_msg()?).await?;
                                    continue 
                                };
                                active_guards.insert(sid, ag);
                                multiplexed_receivers.insert(sid, UnboundedReceiverStream::new(rx));
                            }
                            WsMessage::DropSub { id } => {
                                let sid = StreamId::from_str(&id)?;
                                // Dropping here will drop the AttachedStreamGuard hence also removing the mapping from mesophyll automatically
                                multiplexed_receivers.remove(&sid);
                                active_guards.remove(&sid);
                            }
                            WsMessage::Hb {} | WsMessage::Warn { .. } => {},
                            WsMessage::Msg { id, msg } => {
                                let sid = StreamId::from_str(&id)?;
                                state.worker_pool.stream_message(sid, msg.0)?;
                            }
                        }
                    }
                    Some((id, kv)) = multiplexed_receivers.next() => {
                        ws_sender.send(WsMessage::Msg { id: id.to_hex_str(), msg: CKhronosValue(kv) }.to_msg()?).await?;
                    }
                }
            }
            Ok(())
        }

        ws.on_upgrade(|mut socket| async move {
            if let Err(e) = handle_socket(&mut socket, state).await {
                send_close_message(&mut socket, 4000, &format!("Error occured: {e}")).await;
            } else {
                send_close_message(&mut socket, 1001, "Going away").await;
            }
        })
    }

    let mut router = Router::new();

    router = router
        .route("/healthcheck", post(|| async { Json(()) }))
        .route("/msyscall", post(msyscall))
        .route("/blob", get(get_presigned))
        .route("/ws", get(ws))
        .fallback(get(|| async {
            (
                StatusCode::NOT_FOUND,
                "Use /msyscall for msyscall (insecure) and /msyscall/secure for msyscall (secure, staff-only)"
            )
        }))
        .layer(
            tower_http::cors::CorsLayer::very_permissive()
            .expose_headers([header::RETRY_AFTER])
            .max_age(MaxAge::exact(Duration::from_secs(86400)))
        )
        .layer(axum::middleware::from_fn(logger));

    let router: Router<()> = router.with_state(handler);
    router.into_make_service()
}