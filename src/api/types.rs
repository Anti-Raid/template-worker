use std::collections::HashMap;
use std::time::Duration;

use chrono::DateTime;
use chrono::Utc;
use khronos_runtime::utils::khronos_value::KhronosValue;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::all::ChannelType;
use serenity::all::GuildChannel;
use serenity::all::GuildId;
use serenity::all::InstallationContext;
use serenity::all::Permissions;
use serenity::all::Role;
use serenity::all::RoleId;
use serenity::all::UserId;
use serenity::all::CommandOptionType;
use serenity::all::CommandType;

/// Query parameters for dispatch_event_and_wait
#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct DispatchEventAndWaitQuery {
    /// Wait duration in milliseconds
    pub wait_timeout: Option<u64>,
}

#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ExecuteLuaVmActionOpts {
    pub wait_timeout: Option<std::time::Duration>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct ExecuteLuaVmActionResponse {
    pub results: Vec<ApiLuaVmResultHandle>,
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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SettingsOperationRequest {
    pub fields: Value,
    pub op: String,
    pub setting: String,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema)]
pub struct TwState {
    pub commands: Vec<ApiCreateCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DashboardGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub permissions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct DashboardGuildData {
    pub guilds: Vec<DashboardGuild>,
    pub bot_in_guilds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiConfig {
    /// The ID of the main AntiRaid support server
    #[schema(value_type = String)]
    pub main_server: GuildId,
    /// Discord Support Server Link
    pub support_server_invite: String,
    /// The ID of the AntiRaid bot client
    #[schema(value_type = String)]
    pub client_id: UserId,
}

/// Defines the structure of an authorization request
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct AuthorizeRequest {
    /// Discord Oauth2 code
    pub code: String,
    /// The redirect URI to return to after authorization
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
/// Create a API user session
pub struct CreateUserSession {
    pub name: String,
    pub r#type: String, // Currently must be 'api'
    pub expiry: i64, // Expiry in seconds
}

/// Defines a CreateUserSessionResponse structure which is used to return session information
/// after creation of a session
/// 
/// May contain partial user information if the session was created via OAuth2 login
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
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
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
/// Represents an authorized session and its associated user
/// 
/// Note: this is *very* different from a UserSession and provides different/limited data
pub struct AuthorizedSession {
    /// User ID
    pub user_id: String,
    /// Session ID
    pub id: String,
    /// The state of the user
    pub state: String,
    /// The type of session
    pub r#type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UserSession {
    /// The ID of the session
    pub id: String,
    /// The name of the session
    pub name: Option<String>,
    /// The ID of the user who created the session
    pub user_id: String,
    /// The time the session was created
    pub created_at: DateTime<Utc>,
    /// The type of session (e.g., "login", "api")
    pub r#type: String,
    /// The time the session expires
    pub expiry: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct UserSessionList {
    /// The list of user sessions
    pub sessions: Vec<UserSession>,
}

/// A shard connection (for bot statistics)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ShardConn {
    /// The status of the shard connection
    pub status: String,
    /// The real latency of the shard connection
    pub real_latency: i64,
    /// The number of guilds the shard is connected to
    pub guilds: i64,
    /// The uptime of the shard connection in seconds
    pub uptime: i64,
    /// The total uptime of the shard connection in seconds
    pub total_uptime: i64,
}

/// A response containing the status of all shards
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct GetStatusResponse {
    /// A map of shard group ID to shard connection information
    pub shard_conns: std::collections::HashMap<i64, ShardConn>,
    /// The total number of guilds the bot is connected to
    pub total_guilds: i64,
}

/// Publicly accessible representation of a Discord command
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiCreateCommand {
    #[serde(rename = "type")]
    #[schema(value_type = u8)]
    pub kind: Option<CommandType>,
    pub name: Option<String>,
    pub name_localizations: HashMap<String, String>,
    pub description: Option<String>,
    pub description_localizations: HashMap<String, String>,
    #[schema(value_type = u8)]
    pub integration_types: Option<Vec<InstallationContext>>,
    pub nsfw: bool,
    pub options: Vec<ApiCreateCommandOption>,
}

/// Publicly accessible representation of a Discord command option
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiCreateCommandOption {
    #[serde(rename = "type")]
    #[schema(value_type = u8)]
    pub kind: CommandOptionType,
    pub name: String,
    pub name_localizations: Option<HashMap<String, String>>,
    pub description: String,
    pub description_localizations: Option<HashMap<String, String>>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub choices: Vec<ApiCreateCommandOptionChoice>,
    #[serde(default)]
    pub options: Vec<ApiCreateCommandOption>,
    #[serde(default)]
     #[schema(value_type = u8)]
    pub channel_types: Vec<ChannelType>,
    #[serde(default)]
    #[schema(value_type = u64)]
    pub min_value: Option<serde_json::Number>,
    #[serde(default)]
    #[schema(value_type = u64)]
    pub max_value: Option<serde_json::Number>,
    #[serde(default)]
    pub min_length: Option<u16>,
    #[serde(default)]
    pub max_length: Option<u16>,
    #[serde(default)]
    pub autocomplete: bool,
}

/// Represents a choice for a command option
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ApiCreateCommandOptionChoice {
    pub name: String,
    pub name_localizations: Option<HashMap<String, String>>,
    pub value: Value,
}

#[derive(serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub struct ApiLuaVmResultHandle {
    pub result: ApiLuaVmResult,
    pub template_name: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema)]
pub enum ApiLuaVmResult {
    Ok { 
        #[schema(value_type = serde_json::Value)]
        result: KhronosValue 
    }, // any result can be a json enum
    LuaError { err: String },
    VmBroken {},
}