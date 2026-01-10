use crate::api::auth::create_web_session;
use crate::api::auth::create_web_user_from_oauth2;
use crate::api::auth::delete_user_session;
use crate::api::auth::get_user_sessions;
use crate::api::auth::SessionType;
use crate::api::extractors::AuthorizedUser;
use crate::api::server::ApiResponseError;
use crate::api::types::ApiConfig;
use crate::api::types::ApiCreateCommand;
use crate::api::types::ApiCreateCommandOption;
use crate::api::types::ApiCreateCommandOptionChoice;
use crate::api::types::ApiPartialGuildChannel;
use crate::api::types::ApiPartialRole;
use crate::api::types::AuthorizeRequest;
use crate::api::types::GetStatusResponse;
use crate::api::types::GuildChannelWithPermissions;
use crate::api::types::SettingDispatch;
use crate::api::types::SettingExecuteDispatch;
use crate::api::types::ShardConn;
use crate::api::types::UserSessionList;
use crate::dispatch::parse_response;
use crate::events::AntiraidEvent;
use crate::events::GetSettingsEvent;
use crate::events::SettingExecuteEvent;
use crate::worker::workervmmanager::Id;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
};
use axum::Json;
use chrono::Utc;
use moka::future::Cache;
use serenity::all::UserId;
use std::sync::LazyLock;
use std::{collections::HashMap, sync::Arc};
use sqlx::Row;

use super::types::{
    BaseGuildUserInfo, SettingsOperationRequest, TwState,
    DashboardGuild, DashboardGuildData, PartialUser, CreateUserSessionResponse, AuthorizedSession,
    CreateUserSession, SettingDispatchDocType, SettingExecuteDispatchDocType
};
use crate::dispatch::parse_event;
use super::server::{AppData, ApiResponse, ApiError, ApiErrorCode}; 

static BOT_HAS_GUILD_CACHE: LazyLock<Cache<serenity::all::GuildId, ()>> = LazyLock::new(|| {
    Cache::builder()
        .time_to_live(std::time::Duration::from_secs(120)) // 2 minutes
        .build()
});

/// Helper function to check if the bot is in a guild
async fn check_guild_has_bot(
    data: &crate::data::Data,
    guild_id: serenity::all::GuildId,
) -> Result<(), ApiResponseError> {
    if !BOT_HAS_GUILD_CACHE.contains_key(&guild_id) {
        let guild_exists = crate::sandwich::has_guilds(&data.reqwest, vec![guild_id])
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

        if guild_exists.is_empty() || guild_exists[0] == 0 {
            return Err((StatusCode::NOT_FOUND, Json("Guild to get settings for does not have the bot?".into())));
        }

        BOT_HAS_GUILD_CACHE.insert(guild_id, ()).await;
    }

    Ok(())
}

/// Get Settings For Guild User
/// 
/// Gets the settings for a guild given a user. Note that it is perfectly
/// allowed for the user to not be in the guild itself (e.g. ban appeal type settings
/// in the future)
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/guilds/{guild_id}/settings",
    security(
        ("UserAuth" = []) 
    ),
    params(
        ("guild_id" = String, description = "The ID of the guild to get the user info for")
    ),
    responses(
        (status = 200, description = "Settings for the guild", body = SettingDispatchDocType),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_settings_for_guild_user(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    AuthorizedUser { user_id, .. }: AuthorizedUser, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
) -> ApiResponse<SettingDispatch> {
    // Make a GetSetting event
    let user_id: UserId = user_id.parse()
        .map_err(|e: serenity::all::ParseIdError| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    // Ensure the bot is in the guild
    check_guild_has_bot(&data, guild_id).await?;

    let event = parse_event(&AntiraidEvent::GetSettings(GetSettingsEvent {
        author: user_id,
    }))
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_string().into())))?;

    let results = parse_response(
        data.worker.dispatch_event_to_templates(
            Id::GuildId(guild_id),
            event,
        )
        .await
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?
    .into_iter()
    .map(|(name, result)| (name, result.into()))
    .collect::<HashMap<_, _>>();

    Ok(Json(results))
}

/// Execute Setting For User
///
/// Executes a setting for a guild given a user. Note that it is perfectly
/// allowed for the user to not be in the guild itself (e.g. ban appeal type settings
/// in the future)
#[utoipa::path(
    post, 
    tag = "Public API",
    path = "/guilds/{guild_id}/settings",
    security(
        ("UserAuth" = []) 
    ),
    params(
        ("guild_id" = String, description = "The ID of the guild to get the user info for")
    ),
    responses(
        (status = 200, description = "Settings for the guild", body = SettingExecuteDispatchDocType),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn execute_setting_for_guild_user(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    AuthorizedUser { user_id, .. }: AuthorizedUser, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
    Json(req): Json<SettingsOperationRequest>,
) -> ApiResponse<SettingExecuteDispatch> {
    let user_id: UserId = user_id.parse()
        .map_err(|e: serenity::all::ParseIdError| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    // Ensure the bot is in the guild
    check_guild_has_bot(&data, guild_id).await?;

    let guild_exists = crate::sandwich::has_guilds(&data.reqwest, vec![guild_id])
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    if guild_exists.is_empty() || guild_exists[0] == 0 {
        return Err((StatusCode::NOT_FOUND, Json("Guild to get settings for does not have the bot?".into())));
    }

    let op = req.operation;

    // Make a ExecuteSetting event
    let event = parse_event(&AntiraidEvent::ExecuteSetting(SettingExecuteEvent {
        id: req.setting.clone(),
        op,
        author: user_id,
        fields: req.fields,
    }))
    .map_err(|e| (StatusCode::BAD_REQUEST, Json(e.to_string().into())))?;

    let results = parse_response(
        data.worker.dispatch_scoped_event_to_templates(
            Id::GuildId(guild_id),
            event,
            vec![req.setting],
        )
        .await
    )
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?
    .into_iter()
    .map(|(name, result)| (name, result.into()))
    .collect::<HashMap<_, _>>();

    Ok(Json(results))
}

#[derive(serde::Deserialize, utoipa::ToSchema)]
pub(super) struct GetUserGuildsQuery {
    refresh: Option<bool>,
}

/// Get User Guilds
/// 
/// Returns information about a user's guilds
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/users/@me/guilds",
    security(
        ("UserAuth" = []) 
    ),
    responses(
        (status = 200, description = "The list of the users servers along with which one the bot is in", body = DashboardGuildData),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_user_guilds(
        State(AppData {
        data,
        ..
    }): State<AppData>,
    AuthorizedUser { user_id, session_type, .. }: AuthorizedUser, // Internal endpoint
    Query(GetUserGuildsQuery { refresh }): Query<GetUserGuildsQuery>,
) -> ApiResponse<DashboardGuildData> {
    // TODO: Remove this restriction once we properly refresh the access token upon expiry etc.
    if session_type != "login" { 
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                message: "This endpoint is restricted to only Discord Oauth2 login sessions for now.".to_string(),
                code: ApiErrorCode::Restricted,
            }),
        ));
    }

    let refresh = refresh.unwrap_or(false);

    let mut guilds_cache = None;
    if !refresh {
        // Check for guilds cache
        let cached_guilds = sqlx::query("SELECT guilds_cache FROM users WHERE user_id = $1")
            .bind(&user_id)
            .fetch_one(&data.pool)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

        if let Some(cached_guilds_data) = cached_guilds
        .try_get::<Option<serde_json::Value>, _>("guilds_cache") 
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))? {
            guilds_cache = Some(serde_json::from_value::<Vec<DashboardGuild>>(cached_guilds_data)
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?);
        }
    }

    let guilds = match guilds_cache {
        Some(gc) => gc,
        None => {
            // Get the access token
            #[derive(sqlx::FromRow)]
            struct AccessToken {
                access_token: Option<String>,
            }

            let access_token: AccessToken = sqlx::query_as("SELECT access_token FROM users WHERE user_id = $1")
                .bind(&user_id)
                .fetch_one(&data.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

            // This should never happen but just in case...
            let Some(access_token) = access_token.access_token else {
                return Err((StatusCode::BAD_REQUEST, Json("User has not logged in/authenticated via OAuth2 yet!".into())));
            };

            let resp = data.reqwest.get(format!("{}/api/v10/users/@me/guilds", crate::CONFIG.meta.proxy))
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

            if resp.status() != reqwest::StatusCode::OK {
                let error_text = resp.text().await
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(format!("Failed to get user guilds: {}", error_text).into()),
                ));
            }

            #[derive(serde::Deserialize)]
            pub struct OauthGuild {
                id: String,
                name: String,
                icon: Option<String>,
                permissions: String,
            }

            let guilds: Vec<OauthGuild> = resp.json()
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

            let mut dashboard_guilds = Vec::with_capacity(guilds.len());

            for guild in guilds {
                let dashboard_guild = DashboardGuild {
                    id: guild.id,
                    name: guild.name,
                    icon: guild.icon,
                    permissions: guild.permissions,
                };

                dashboard_guilds.push(dashboard_guild);
            }

            // Now update the database
            sqlx::query("UPDATE users SET guilds_cache = $1 WHERE user_id = $2")
                .bind(serde_json::to_value(&dashboard_guilds)
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?)
                .bind(&user_id)
                .execute(&data.pool)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

            dashboard_guilds
        }
    };

    let mut guild_ids = Vec::with_capacity(guilds.len());
    for guild in guilds.iter() {
        guild_ids.push(guild.id.parse::<serenity::all::GuildId>()
            .map_err(|e: serenity::all::ParseIdError| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?);
    }

    let guilds_exist = crate::sandwich::has_guilds(
        &data.reqwest,
        guild_ids.clone(),
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    if guilds_exist.len() != guilds.len() {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(format!("Gateway did not return all guilds: expected {}, got {}", guilds.len(), guilds_exist.len()).into()),
        ));
    }

    let mut bot_in_guilds = Vec::with_capacity(guilds.len());
    for (i, exists) in guilds_exist.into_iter().enumerate() {
        if exists == 1 {
            bot_in_guilds.push(guild_ids[i].to_string());
        }
    }

    Ok(Json(DashboardGuildData {
        guilds,
        bot_in_guilds,
    }))
}

/// Base Guild User Info
/// 
/// Returns basic user/guild information
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/users/@me/guilds/{guild_id}",
    security(
        ("UserAuth" = []) 
    ),
    params(
        ("guild_id" = String, description = "The ID of the guild to get the user info for")
    ),
    responses(
        (status = 200, description = "Basic data about the guild", body = BaseGuildUserInfo),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn base_guild_user_info(
    State(AppData {
        data,
        http,
        ..
    }): State<AppData>,
    AuthorizedUser { user_id, .. }: AuthorizedUser, // Internal endpoint
    Path(guild_id): Path<serenity::all::GuildId>,
) -> ApiResponse<BaseGuildUserInfo> {
    let user_id: UserId = user_id.parse()
        .map_err(|e: serenity::all::ParseIdError| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    let bot_user_id = data.current_user.id;
    let guild_json = crate::sandwich::guild(
        &http,
        &data.reqwest,
        guild_id,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(format!("Failed to get guild: {:#?}", e).into()),
        )
    })?;

    let guild = serde_json::from_value::<serenity::all::PartialGuild>(guild_json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    // Next fetch the member and bot_user
    let member_json = match crate::sandwich::member_in_guild(
        &http,
        &data.reqwest,
        guild_id,
        user_id,
    )
    .await
    {
        Ok(Some(member)) => member,
        Ok(None) => {
            return Err((StatusCode::NOT_FOUND, Json("User not found in server".into())));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(format!("Failed to get member: {:#?}", e).into()),
            ));
        }
    };

    let member = serde_json::from_value::<serenity::all::Member>(member_json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    let bot_user_json = match crate::sandwich::member_in_guild(
        &http,
        &data.reqwest,
        guild_id,
        bot_user_id,
    )
    .await
    {
        Ok(Some(member)) => member,
        Ok(None) => {
            return Err((StatusCode::NOT_FOUND, Json("Bot user not found".into())));
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(format!("Failed to get bot user: {:#?}", e).into()),
            ));
        }
    };

    let bot_user = serde_json::from_value::<serenity::all::Member>(bot_user_json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    // Fetch the channels
    let channels_json = crate::sandwich::guild_channels(
        &http,
        &data.reqwest,
        guild_id,
    )
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(format!("Failed to get channels: {:#?}", e).into()),
        )
    })?;

    let channels = serde_json::from_value::<Vec<serenity::all::GuildChannel>>(channels_json)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(e.to_string().into())))?;

    let mut channels_with_permissions = Vec::with_capacity(channels.len());

    for channel in channels.iter() {
        channels_with_permissions.push(GuildChannelWithPermissions {
            user: guild.user_permissions_in(channel, &member),
            bot: guild.user_permissions_in(channel, &bot_user),
            channel: ApiPartialGuildChannel {
                id: channel.id.widen(),
                name: channel.base.name.to_string(),
                position: channel.position,
                parent_id: channel.parent_id.map(|id| id.widen()),
                r#type: channel.base.kind.0,
            },
        });
    }

    Ok(Json(BaseGuildUserInfo {
        name: guild.name.to_string(),
        icon: guild.icon_url(),
        owner_id: guild.owner_id.to_string(),
        roles: guild.roles.into_iter().map(|role| {
            ApiPartialRole {
                id: role.id,
                name: role.name.to_string(),
                position: role.position,
                permissions: role.permissions,
            }
        }).collect(),
        user_roles: member.roles.to_vec(),
        bot_roles: bot_user.roles.to_vec(),
        channels: channels_with_permissions,
    }))
}

/// To ensure clients accidentally reusing codes
/// 
/// NOTE: This is not a security mechanism
static OAUTH2_CODE_CACHE: LazyLock<Cache<String, ()>> = LazyLock::new(|| {
    Cache::builder()
        .time_to_live(std::time::Duration::from_secs(60 * 10)) // 10 minutes
        .build()
});

/// Create OAuth2 Session
/// 
/// Creates a login token from a Discord OAuth2 login 
#[utoipa::path(
    post, 
    tag = "Public API",
    path = "/oauth2",
    responses(
        (status = 200, description = "The created session", body = CreateUserSessionResponse),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn create_oauth2_session(
    State(AppData {
        data,
        ..
    }): State<AppData>,
    Json(req): Json<AuthorizeRequest>,
) -> ApiResponse<CreateUserSessionResponse> {
    if !crate::CONFIG.discord_auth.allowed_redirects.contains(&req.redirect_uri) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                message: "This redirect URI is not allowed".to_string(),
                code: ApiErrorCode::Restricted,
            }),
        ));
    }

    if req.code.len() < 3 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                message: "Invalid code specified".to_string(),
                code: ApiErrorCode::InvalidToken,
            }),
        ));
    }

    if OAUTH2_CODE_CACHE.contains_key(&req.code) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                message: "Code has been clearly used before and is as such invalid".to_string(),
                code: ApiErrorCode::InvalidToken,
            }),
        ));
    }

    OAUTH2_CODE_CACHE.insert(req.code.clone(), ()).await;

    let resp = data.reqwest.post(format!("{}/api/v10/oauth2/token", crate::CONFIG.meta.proxy))
        .form(&[
            ("client_id", &data.current_user.id.to_string()),
            ("client_secret", &crate::CONFIG.discord_auth.client_secret),
            ("grant_type", &"authorization_code".to_string()),
            ("code", &req.code),
            ("redirect_uri", &req.redirect_uri),
        ])
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to get access token: {e:?}").into())))?;

    if resp.status() != reqwest::StatusCode::OK {
        let error_text = resp.text().await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to get access token: {e:?}").into())))?;

        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(format!("Failed to get access token: {error_text}").into()),
        ));
    }

    #[derive(serde::Deserialize)]
    pub struct OauthTokenResponse {
        pub access_token: String,
        pub scope: String,
    }

    let token_response: OauthTokenResponse = resp.json()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to parse access token response: {e:?}").into())))?;

    let scopes = token_response.scope.replace("%20", " ")
        .split(' ')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    if !scopes.contains(&"identify".to_string()) || !scopes.contains(&"guilds".to_string()) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                message: "This endpoint requires the 'identify' and 'guilds' scope to be present".to_string(),
                code: ApiErrorCode::InvalidToken,
            }),
        ));
    }    

    // Fetch user info
    let user_resp = data.reqwest.get(format!("{}/api/v10/users/@me", crate::CONFIG.meta.proxy))
        .header("Authorization", format!("Bearer {}", &token_response.access_token))
        .send()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to get user info: {e:?}").into())))?;

    if user_resp.status() != reqwest::StatusCode::OK {
        let error_text = user_resp.text().await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to get user info: {e:?}").into())))?;     
        
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(format!("Failed to get user info: {error_text}").into()),
        ));
    }

    let user_info: PartialUser = user_resp.json()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to parse user info response: {e:?}").into())))?;

    // Create a session for the user
    create_web_user_from_oauth2(
        &data.pool,
        &user_info.id,
        &token_response.access_token,
    ).await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to create user: {e:?}").into())))?;

    let session = create_web_session(
        &data.pool,
        &user_info.id,
        None, // No name for the session
        SessionType::Login,
    )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to create session: {e:?}").into())))?;

    Ok(
        Json(
            CreateUserSessionResponse {
                user_id: user_info.id.clone(),
                token: session.token,
                session_id: session.session_id,
                expiry: session.expires_at,
                user: Some(user_info)
            }
        )
    ) 
}

/// Get Authorized Session
/// 
/// Returns data about both the user and the user's authorized session
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/sessions/@me",
    security(
        ("UserAuth" = []) 
    ),
    responses(
        (status = 200, description = "The authorized session + user data", body = AuthorizedSession),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_authorized_session(
    State(AppData { .. }): State<AppData>,
    AuthorizedUser { user_id, session_id, state, session_type, .. }: AuthorizedUser, // Internal endpoint
) -> ApiResponse<AuthorizedSession> {
    Ok(
        Json(
            AuthorizedSession {
                user_id,
                id: session_id,
                state,
                r#type: session_type,
            }
        )
    )
}

/// Get User Sessions
/// 
/// Returns a list of sessions for the user. Note that session tokens are not returned
/// for security reasons.
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/sessions",
    security(
        ("UserAuth" = []) 
    ),
    responses(
        (status = 200, description = "List of user sessions", body = UserSessionList),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_user_sessions_api(
    State(AppData { data, .. }): State<AppData>,
    AuthorizedUser { user_id, .. }: AuthorizedUser, // Internal endpoint
) -> ApiResponse<UserSessionList> {
    let sessions = get_user_sessions(&data.pool, &user_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to get user sessions: {e:?}").into())))?;

    Ok(Json(UserSessionList { sessions }))
}

/// Create User Session
/// 
/// Creates a new user session. Currently only API tokens can be generated
/// using this endpoint
#[utoipa::path(
    post, 
    tag = "Public API",
    path = "/sessions",
    security(
        ("UserAuth" = []) 
    ),
    responses(
        (status = 200, description = "The created session", body = CreateUserSessionResponse),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn create_user_session(
    State(AppData { data, .. }): State<AppData>,
    AuthorizedUser { user_id, .. }: AuthorizedUser, // Internal endpoint
    Json(req): Json<CreateUserSession>,
) -> ApiResponse<CreateUserSessionResponse> {
    if req.r#type != "api" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ApiError {
                message: "Only 'api' session type is allowed".to_string(),
                code: ApiErrorCode::Restricted,
            }),
        ));
    }

    // Panics when seconds is more than i64::MAX / 1_000 or less than -i64::MAX / 1_000 (in this context, this is the same as i64::MIN / 1_000 due to rounding).
    if req.expiry <= 0 || req.expiry >= i64::MAX / 1_000 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ApiError {
                message: format!("Expiry time must be between 0 and {}", i64::MAX / 1_000),
                code: ApiErrorCode::InvalidToken,
            }),
        ));
    }

    let session = create_web_session(
        &data.pool,
        &user_id,
        Some(req.name),
        SessionType::Api {
            expires_at: Utc::now() + chrono::Duration::seconds(req.expiry),
        },
    )
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to create session: {e:?}").into())))?;

    Ok(Json(CreateUserSessionResponse {
        user_id,
        token: session.token,
        session_id: session.session_id,
        expiry: session.expires_at,
        user: None,
    }))
}

/// Delete User Session
///
/// Deletes a user session by its session ID assuming it is owned by the user. This is useful for logging out a user or deleting unknown/malicious sessions.
#[utoipa::path(
    delete, 
    tag = "Public API",
    path = "/sessions/{session_id}",
    security(
        ("UserAuth" = []) 
    ),
    responses(
        (status = 204, description = "The session was deleted successfully"),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn delete_user_session_api(
    State(AppData { data, .. }): State<AppData>,
    AuthorizedUser { user_id, .. }: AuthorizedUser,
    Path(session_id): Path<String>, // Session ID to delete
) -> ApiResponse<()> {
    delete_user_session(&data.pool, &user_id, &session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to delete user session: {e:?}").into())))?;

    Ok(Json(()))
} 

static STATE_CACHE: std::sync::LazyLock<Arc<TwState>> = std::sync::LazyLock::new(|| {
    fn command_option_choice_into_api_command_option_choice(
        choice: crate::register::CreateCommandOptionChoice,
    ) -> ApiCreateCommandOptionChoice {
        ApiCreateCommandOptionChoice {
            name: choice.name,
            name_localizations: choice.name_localizations,
            value: choice.value,
        }
    }
    
    fn command_option_into_api_command_option(option: crate::register::CreateCommandOption) -> ApiCreateCommandOption {
        ApiCreateCommandOption {
            kind: option.kind,
            name: option.name,
            name_localizations: option.name_localizations,
            description: option.description,
            description_localizations: option.description_localizations,
            required: option.required,
            options: option.options.into_iter().map(command_option_into_api_command_option).collect(),
            channel_types: option.channel_types,
            min_value: option.min_value,
            max_value: option.max_value,
            min_length: option.min_length,
            max_length: option.max_length,
            choices: option.choices.into_iter().map(command_option_choice_into_api_command_option_choice).collect(),
            autocomplete: option.autocomplete
        }
    }
    
    fn command_into_api_command(command: crate::register::CreateCommand) -> ApiCreateCommand {
        ApiCreateCommand {
            kind: command.kind,
            name: command.name,
            name_localizations: command.name_localizations,
            description: command.description,
            description_localizations: command.description_localizations,
            integration_types: command.integration_types,
            nsfw: command.nsfw,
            options: command.options.into_iter().map(command_option_into_api_command_option).collect(),
        }
    }
    
    let state = TwState {
        commands: crate::register::REGISTER.commands.iter()
            .map(|cmd| command_into_api_command(cmd.clone()))
            .collect(),
    };

    Arc::new(state)
});

/// Get Bot State
/// 
/// Returns the list of core/builtin commands of the bot
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/bot-state",
    responses(
        (status = 200, description = "The bot's state", body = TwState),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn state() -> Json<Arc<TwState>> {
    Json(STATE_CACHE.clone())
}

/// Get API Configuration
/// 
/// Returns the base API configuration
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/config",
    responses(
        (status = 200, description = "The base API configuration", body = ApiConfig),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn api_config() -> Json<ApiConfig> {
    Json(ApiConfig {
        main_server: crate::CONFIG.servers.main,
        client_id: crate::CONFIG.discord_auth.client_id,
        support_server_invite: 
        crate::CONFIG.meta.support_server_invite.clone(),
    })
}

static STATS_CACHE: std::sync::LazyLock<Cache<(), GetStatusResponse>> = std::sync::LazyLock::new(|| {
    Cache::builder()
        .time_to_live(std::time::Duration::from_secs(100)) // 1 minute
        .build()
});

/// Get Bot Stats
/// 
/// Returns the bot's stats
#[utoipa::path(
    get, 
    tag = "Public API",
    path = "/bot-stats",
    responses(
        (status = 200, description = "The bot's state", body = GetStatusResponse),
        (status = 400, description = "API Error", body = ApiError),
    )
)]
pub(super) async fn get_bot_stats(
    State(AppData { data, .. }): State<AppData>,
) -> ApiResponse<GetStatusResponse> {
    let stats = STATS_CACHE.get(&()).await;

    if let Some(stats) = stats {
        return Ok(Json(stats));
    }

    let sandwich_raw_stats = crate::sandwich::get_status(&data.reqwest)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(format!("Failed to get bot stats: {e:?}").into())))?;

    let mut total_guilds = 0;
    for shard in sandwich_raw_stats.shard_conns.values() {
        total_guilds += shard.guilds;
    }

    let stats = GetStatusResponse {
        shard_conns: sandwich_raw_stats.shard_conns.into_iter().map(|(id, shard)| {
            (id, ShardConn {
                status: shard.status,
                real_latency: shard.real_latency,
                guilds: shard.guilds,   
                uptime: shard.uptime,
                total_uptime: shard.total_uptime,
            })
        }).collect(),
        total_guilds,
        total_users: sandwich_raw_stats.total_members,
    };

    STATS_CACHE.insert((), stats.clone()).await;

    Ok(Json(stats))
}
