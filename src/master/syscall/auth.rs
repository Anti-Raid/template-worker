use chrono::Utc;
use khronos_ext::mluau_ext::prelude::*;
use serde::{Deserialize, Serialize};
use serenity::all::UserId;
use crate::master::syscall::{MSyscallContext, MSyscallError, MSyscallHandler, internal::auth as iauth, types::auth::UserSession};
use super::types::discord::*;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MAuthSyscall {
    /// Creates a login session using oauth2
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

impl FromLua for MAuthSyscall {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "SyscallArgs".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"CreateLoginSession" => {
                let code = tab.get("code")?;
                let redirect_uri = tab.get("redirect_uri")?;
                let code_verifier = tab.get("code_verifier")?;
                Ok(Self::CreateLoginSession { code, redirect_uri, code_verifier })
            },
            b"CreateApiSession" => {
                let name = tab.get("name")?;
                let expiry = tab.get("expiry")?;
                Ok(Self::CreateApiSession { name, expiry })
            },
            b"GetUserSessions" => {
                Ok(Self::GetUserSessions {})
            },
            b"DeleteSession" => {
                let session_id = tab.get("session_id")?;
                Ok(Self::DeleteSession { session_id })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "MAuthSyscall".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum MAuthSyscallRet {
    /// A created session returned by a syscall
    CreatedSession {
        /// Session metadata
        session: UserSession,
        /// Session token
        token: String,
        /// The user who created the session (only sent on OAuth2 login)
        user: Option<PartialUser>,
    },
    UserSessions {
        sessions: Vec<UserSession>
    },
    Ack
}

impl IntoLua for MAuthSyscallRet {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 5)?;
        match self {
            Self::CreatedSession { session, token, user } => {
                table.set("op", "Session")?;
                table.set("session", session)?;
                table.set("token", token)?;
                table.set("user", user)?;
            }
            Self::UserSessions { sessions } => {
                table.set("op", "UserSessions")?;
                table.set("sessions", sessions)?;
            }
            Self::Ack => {
                table.set("op", "Ack")?;
            }
        }
        Ok(LuaValue::Table(table))
    }
}

#[derive(Serialize, Debug)]
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
                    let error_text = user_resp.text().await?;
                    return Err(format!("Failed to get user info: {}", error_text).into());
                }

                let user_info: PartialUser = user_resp.json().await?;

                // Create a session for the user
                iauth::create_web_user_from_oauth2(
                    &handler.pool,
                    &user_info.id,
                    &token_response.access_token,
                ).await
                .map_err(|e| format!("Failed to create user: {e:?}"))?;

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
                    .map_err(|e| format!("Failed to create session: {e:?}"))?;

                Ok(
                    MAuthSyscallRet::CreatedSession { 
                        session: UserSession {
                            id: session.session_id,
                            user_id: user_info.id.clone(),
                            name: None,
                            created_at: Utc::now(),
                            expiry: session.expires_at,
                            r#type: "login".to_string(),
                        },
                        token: session.token,
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
                    MAuthSyscallRet::CreatedSession {
                        session: UserSession {
                            id: session.session_id,
                            user_id: user_id.to_string(),
                            name: None,
                            created_at: Utc::now(),
                            expiry: session.expires_at,
                            r#type: "api".to_string(),
                        },
                        token: session.token,
                        user: None
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
                Ok(MAuthSyscallRet::Ack)
            }
        }
    }
}
