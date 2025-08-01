use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::all::GuildChannel;
use serenity::all::Permissions;
use serenity::all::Role;
use serenity::all::RoleId;

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
