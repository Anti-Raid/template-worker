use std::collections::HashMap;

use chrono::DateTime;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::all::ChannelType;
use serenity::all::GenericChannelId;
use serenity::all::GuildId;
use serenity::all::InstallationContext;
use serenity::all::Permissions;
use serenity::all::RoleId;
use serenity::all::UserId;
use serenity::all::CommandOptionType;
use serenity::all::CommandType;
use ts_rs::TS;
use khronos_runtime::utils::khronos_value::KhronosValue;

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct GuildChannelWithPermissions {
    #[schema(value_type = String)]
    #[ts(as = "String")]
    /// User permissions
    pub user: Permissions,
    #[schema(value_type = String)]
    #[ts(as = "String")]
    /// Bot permissions
    pub bot: Permissions,
    /// Channel data
    pub channel: ApiPartialGuildChannel,
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct ApiPartialGuildChannel {
    #[schema(value_type = String)]
    #[ts(as = "String")]
    /// The ID of the channel
    pub id: GenericChannelId,
    /// The name of the channel
    pub name: String,
    /// The position of the channel in the guild
    pub position: u16,
    /// The ID of the parent channel, if any
    #[schema(value_type = Option<String>)]
    #[ts(as = "Option<String>")]
    pub parent_id: Option<GenericChannelId>,
    #[schema(value_type = u8)]
    /// The type of the channel
    pub r#type: u8,
}

#[derive(Debug, Serialize, Deserialize, Clone, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct ApiPartialRole {
    #[schema(value_type = String)]
    #[ts(as = "String")]
    /// The ID of the role
    pub id: RoleId,
    /// The name of the role
    pub name: String,
    /// The position of the role in the guild
    pub position: i16,
    /// Permissions of the role
    #[schema(value_type = String)]
    #[ts(as = "String")]
    pub permissions: Permissions,
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct BaseGuildUserInfo {
    pub owner_id: String,
    pub name: String,
    pub icon: Option<String>,
    /// List of all roles in the server
    pub roles: Vec<ApiPartialRole>,
    /// List of roles the user has
    #[schema(value_type = Vec<String>)]
    #[ts(as = "Vec<String>")]
    pub user_roles: Vec<RoleId>,
    /// List of roles the bot has
    #[schema(value_type = Vec<String>)]
    #[ts(as = "Vec<String>")]
    pub bot_roles: Vec<RoleId>,
    /// List of all channels in the server
    pub channels: Vec<GuildChannelWithPermissions>,
}

#[derive(Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct TwState {
    pub commands: Vec<ApiCreateCommand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct DashboardGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub permissions: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct DashboardGuildData {
    pub guilds: Vec<DashboardGuild>,
    pub bot_in_guilds: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct ApiConfig {
    /// The ID of the main AntiRaid support server
    #[schema(value_type = String)]
    #[ts(as = "String")]
    pub main_server: GuildId,
    /// Discord Support Server Link
    pub support_server_invite: String,
    /// The ID of the AntiRaid bot client
    #[schema(value_type = String)]
    #[ts(as = "String")]
    pub client_id: UserId,
}

/// Defines the structure of an authorization request
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct AuthorizeRequest {
    /// Discord Oauth2 code
    pub code: String,
    /// The redirect URI to return to after authorization
    pub redirect_uri: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
/// Create a API user session
pub struct CreateUserSession {
    pub name: String,
    pub r#type: String, // Currently must be 'api'
    #[ts(type = "number")]
    pub expiry: i64, // Expiry in seconds
}

/// Defines a CreateUserSessionResponse structure which is used to return session information
/// after creation of a session
/// 
/// May contain partial user information if the session was created via OAuth2 login
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
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
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
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

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct UserSessionList {
    /// The list of user sessions
    pub sessions: Vec<UserSession>,
}

/// A shard connection (for bot statistics)
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct ShardConn {
    /// The status of the shard connection
    pub status: String,
    /// The real latency of the shard connection
    #[ts(type = "number")]
    pub real_latency: i64,
    /// The number of guilds the shard is connected to
    #[ts(type = "number")]
    pub guilds: i64,
    /// The uptime of the shard connection in seconds
    #[ts(type = "number")]
    pub uptime: i64,
    /// The total uptime of the shard connection in seconds
    #[ts(type = "number")]
    pub total_uptime: i64,
}

/// A response containing the status of all shards
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct GetStatusResponse {
    /// A map of shard group ID to shard connection information
    #[ts(as = "HashMap<i32, ShardConn>")]
    pub shard_conns: HashMap<i64, ShardConn>,
    /// The total number of guilds the bot is connected to
    #[ts(type = "number")]
    pub total_guilds: i64,
}

/// Publicly accessible representation of a Discord command
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct ApiCreateCommand {
    #[serde(rename = "type")]
    #[schema(value_type = u8)]
    #[ts(as = "u8")]
    pub kind: Option<CommandType>,
    pub name: Option<String>,
    pub name_localizations: HashMap<String, String>,
    pub description: Option<String>,
    pub description_localizations: HashMap<String, String>,
    #[schema(value_type = u8)]
    #[ts(as = "u8")]
    pub integration_types: Option<Vec<InstallationContext>>,
    pub nsfw: bool,
    pub options: Vec<ApiCreateCommandOption>,
}

/// Publicly accessible representation of a Discord command option
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct ApiCreateCommandOption {
    #[serde(rename = "type")]
    #[schema(value_type = u8)]
    #[ts(as = "u8")]
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
    #[schema(no_recursion)]
    pub options: Vec<ApiCreateCommandOption>,
    #[serde(default)]
    #[schema(value_type = u8)]
    #[ts(as = "u8")]
    pub channel_types: Vec<ChannelType>,
    #[serde(default)]
    #[schema(value_type = u64)]
    #[ts(as = "Option<u32>")]
    pub min_value: Option<serde_json::Number>,
    #[serde(default)]
    #[schema(value_type = Option<u64>)]
    #[ts(as = "Option<u32>")]
    pub max_value: Option<serde_json::Number>,
    #[serde(default)]
    pub min_length: Option<u16>,
    #[serde(default)]
    pub max_length: Option<u16>,
    #[serde(default)]
    pub autocomplete: bool,
}

/// Represents a choice for a command option
#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct ApiCreateCommandOptionChoice {
    pub name: String,
    pub name_localizations: Option<HashMap<String, String>>,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct PublicLuauExecute {
    /// The event name, must start with 'Web' for security reasons
    pub name: String,

    #[schema(value_type = KhronosValueApi)]
    #[ts(as = "KhronosValueApi")]
    /// The event data
    pub data: KhronosValue,
}

// Type for documentation and TypeScript generation purposes
#[derive(Debug, Serialize, Deserialize, TS, utoipa::ToSchema)]
pub enum KhronosValueApi {
    Text(String),
    Integer(i64),
    UnsignedInteger(u64),
    Float(f64),
    Boolean(bool),
    Buffer(Vec<u8>),   
    Vector((f32, f32, f32)), 
    Map(Vec<(KhronosValueApi, KhronosValueApi)>),
    List(Vec<KhronosValueApi>),
    Timestamptz(chrono::DateTime<chrono::Utc>),
    Interval(chrono::Duration),
    TimeZone(String),
    LazyStringMap(HashMap<String, String>), 
    Null,
}
