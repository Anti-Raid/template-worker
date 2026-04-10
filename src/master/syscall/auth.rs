use chrono::{DateTime, Utc};
use serde::Serialize;
use serenity::all::UserId;
use sqlx::Row;
use crate::master::syscall::{MSyscallContext, MSyscallError, MSyscallHandler, internal::auth as iauth, types::auth::UserSession};
use super::types::discord::*;

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum MAuthSyscall {
    CreateLoginSession {
        code: String,
        redirect_uri: String,
        code_verifier: Option<String>,
    },
    CreateApiSession {
        name: String,
        expiry: i64 // expiry in seconds
    },
    GetUserSessions {},
    DeleteSession { session_id: String }
}

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum MAuthSyscallRet {
    /// A created session returned by a syscall
    Session {
        /// The ID of the user who created the session
        user_id: String,
        /// The token of the session
        token: String,
        /// The ID of the session
        session_id: String,
        /// The time the session expires
        expiry: DateTime<Utc>,
        /// The user who created the session (only sent on OAuth2 login)
        user: Option<PartialUser>,
    },
    UserSessions {
        sessions: Vec<UserSession>
    },
    Success
}

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum AuthError {
    /// Invalid redirect URI not allowed by server
    InvalidRedirectUri,
    /// Code too short (invalid)
    CodeTooShort,
    /// Code has been reused in the past couple minutes, most likely invalid, reauth needed
    CodeReuseDetected,
    /// Oauth requires 'identify' and 'guilds' scope but a needed scope was not found
    NeededScopesNotFound,
    /// Expiry time out of range (for creating api sessions etc)
    ExpiryTimeOutOfRange
}

impl MAuthSyscall {
    // For App
    const APP_OAUTH2_REDIRECT_URI: &str = "antiraid://oauth-callback";
    pub(super) async fn exec(self, handler: &MSyscallHandler, ctx: MSyscallContext) -> Result<MAuthSyscallRet, MSyscallError> {
        match self {
            Self::CreateLoginSession { code, redirect_uri, code_verifier } => {
                if !crate::CONFIG.discord_auth.allowed_redirects.contains(&redirect_uri) {
                    return Err(MSyscallError::AuthError { reason: AuthError::InvalidRedirectUri });
                }

                if code.len() < 3 {
                    return Err(MSyscallError::AuthError { reason: AuthError::CodeTooShort });
                }

                if handler.oauth2_code_cache.contains_key(&code) {
                    return Err(MSyscallError::AuthError { reason: AuthError::CodeReuseDetected });
                }

                handler.oauth2_code_cache.insert(code.clone(), ()).await;

                let app_login = redirect_uri == Self::APP_OAUTH2_REDIRECT_URI && code_verifier.is_some();

                #[derive(serde::Serialize)]
                pub struct Response<'a> {
                    client_id: UserId,
                    client_secret: &'a str,
                    grant_type: &'static str,
                    code: String,
                    redirect_uri: String,
                    #[serde(skip_serializing_if = "Option::is_none")]
                    code_verifier: Option<String>,
                }

                let resp = handler.reqwest.post(format!("{}/api/v10/oauth2/token", crate::CONFIG.meta.proxy))
                    .form(&Response {
                        client_id: handler.current_user.id,
                        client_secret: &crate::CONFIG.discord_auth.client_secret,
                        grant_type: "authorization_code",
                        code,
                        redirect_uri,
                        code_verifier,
                    })
                    .send()
                    .await
                    .map_err(|e| format!("Failed to get access token: {e:?}"))?;

                if resp.status() != reqwest::StatusCode::OK {
                    let error_text = resp.text().await?;
                    return Err(format!("Failed to get access token: {}", error_text).into());
                }

                #[derive(serde::Deserialize)]
                pub struct OauthTokenResponse {
                    pub access_token: String,
                    pub scope: String,
                }

                let token_response: OauthTokenResponse = resp.json().await?;

                let scopes = token_response.scope.replace("%20", " ")
                    .split(' ')
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>();

                if !scopes.contains(&"identify".to_string()) || !scopes.contains(&"guilds".to_string()) {
                    return Err(MSyscallError::AuthError { reason: AuthError::NeededScopesNotFound });
                }    

                // Fetch user info
                let user_resp = handler.reqwest.get(format!("{}/api/v10/users/@me", crate::CONFIG.meta.proxy))
                    .header("Authorization", format!("Bearer {}", &token_response.access_token))
                    .send()
                    .await
                    .map_err(|e| format!("Failed to get user info from discord: {e:?}"))?;

                if user_resp.status() != reqwest::StatusCode::OK {
                    let error_text = resp.text().await?;
                    return Err(format!("Failed to get user info: {}", error_text).into());
                }

                let user_info: PartialUser = user_resp.json().await?;

                // Create a session for the user
                iauth::create_web_user_from_oauth2(
                    &handler.pool,
                    &user_info.id,
                    &token_response.access_token,
                ).await
                .map_err(|e| format!("Failed to create user: {e:?}").into())?;

                let session = iauth::create_web_session(
                    &handler.pool,
                    &user_info.id,
                    None, // No name for the session
                    if app_login {
                        iauth::SessionType::AppLogin
                    } else {
                        iauth::SessionType::Login
                    },
                )
                    .await
                    .map_err(|e| format!("Failed to create session: {e:?}").into())?;

                Ok(
                    MAuthSyscallRet::Session { 
                        user_id: user_info.id,
                        token: session.token,
                        session_id: session.session_id,
                        expiry: session.expires_at,
                        user: Some(user_info)
                    }
                ) 
            }
            Self::CreateApiSession { name, expiry } => {
                let user_id = ctx.into_user_id()?;

                // Panics when seconds is more than i64::MAX / 1_000 or less than -i64::MAX / 1_000 (in this context, this is the same as i64::MIN / 1_000 due to rounding).
                if expiry <= 0 || expiry >= i64::MAX / 1_000 {
                    return Err(MSyscallError::AuthError { reason: AuthError::ExpiryTimeOutOfRange });
                }

                let session = iauth::create_web_session(
                    &handler.pool,
                    &user_id.to_string(),
                    Some(name),
                    iauth::SessionType::Api {
                        expires_at: Utc::now() + chrono::Duration::seconds(expiry),
                    },
                )
                .await?;

                Ok(
                    MAuthSyscallRet::Session {
                        user_id: user_id.to_string(),
                        token: session.token,
                        session_id: session.session_id,
                        expiry: session.expires_at,
                        user: None,
                    }
                )
            }
            Self::GetUserSessions {  } => {
                let user_id = ctx.into_user_id()?;
                let sessions = iauth::get_user_sessions(&handler.pool, &user_id.to_string()).await?;
                Ok(MAuthSyscallRet::UserSessions { sessions })
            }
            Self::DeleteSession { session_id } => {
                let user_id = ctx.into_user_id()?;
                iauth::delete_user_session(&handler.pool, &user_id.to_string(), &session_id).await?;
                Ok(MAuthSyscallRet::Success)
            }
        }
    }
}
