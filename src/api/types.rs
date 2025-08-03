use std::time::Duration;

use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::all::GuildChannel;
use serenity::all::GuildId;
use serenity::all::Permissions;
use serenity::all::Role;
use serenity::all::RoleId;
use serenity::all::UserId;

/// Query parameters for dispatch_event_and_wait
#[derive(serde::Serialize, serde::Deserialize)]
pub struct DispatchEventAndWaitQuery {
    /// Wait duration in milliseconds
    pub wait_timeout: Option<u64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ExecuteLuaVmActionOpts {
    pub wait_timeout: Option<std::time::Duration>,
}

#[derive(Serialize, Deserialize)]
pub struct ExecuteLuaVmActionResponse {
    pub data: crate::templatingrt::MultiLuaVmResultHandle,
    pub time_taken: Duration,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GuildChannelWithPermissions {
    pub user: Permissions,
    pub bot: Permissions,
    pub channel: GuildChannel,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BaseGuildUserInfo {
    pub owner_id: String,
    pub name: String,
    pub icon: Option<String>,
    /// List of all roles in the server
    pub roles: Vec<Role>,
    /// List of roles the user has
    pub user_roles: Vec<RoleId>,
    /// List of roles the bot has
    pub bot_roles: Vec<RoleId>,
    /// List of all channels in the server
    pub channels: Vec<GuildChannelWithPermissions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsOperationRequest {
    pub fields: Value,
    pub op: String,
    pub setting: String,
}

#[derive(Serialize, Deserialize)]
pub struct TwState {
    pub commands: Vec<crate::register::CreateCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub permissions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardGuildData {
    pub guilds: Vec<DashboardGuild>,
    pub bot_in_guilds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    /// The ID of the main AntiRaid support server
    pub main_server: GuildId,
    /// Discord Support Server Link
    pub support_server_invite: String,
    /// The ID of the AntiRaid bot client
    pub client_id: UserId,
}

/// Defines the structure of an authorization request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizeRequest {
    /// Discord Oauth2 code
    pub code: String,
    /// The redirect URI to return to after authorization
    pub redirect_uri: String,
}

/// Defines a CreateUserSessionResponse structure which is used to return session information
/// after creation of a session
/// 
/// May contain partial user information if the session was created via OAuth2 login
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserSessionResponse {
    /// The ID of the user who created the session
    pub user_id: String,
    /// The token of the session
    pub token: String,
    /// The ID of the session
    pub session_id: String,
    /// The time the session expires
    pub expiry: DateTime<Utc>,
    /// The user who created the session (only sent on OAuth2 login)
    pub user: Option<PartialUser>,
}

/// The PartialUser of a user, which contains only the necessary fields for the API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialUser {
    /// The ID of the user
    pub id: String,
    /// The username of the user
    pub username: String,
    /// The global name of the user
    pub global_name: Option<String>,
    /// The avatar hash of the user
    pub avatar: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthorizedSession {
    pub user_id: String,
    pub session_id: String,
    pub state: String,
    pub session_type: String,
}