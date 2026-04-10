use serde::{Deserialize, Serialize};
use serenity::all::{GenericChannelId, Permissions, RoleId};
use khronos_ext::mluau_ext::prelude::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub permissions: String,
    pub owner: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DashboardGuildData {
    pub guilds: Vec<DashboardGuild>,
    pub guilds_exist: Vec<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct GuildChannelWithPermissions {
    /// User permissions
    pub user: Permissions,
    /// Bot permissions
    pub bot: Permissions,
    /// Channel data
    pub channel: ApiPartialGuildChannel,
}

#[derive(Debug, Serialize, Clone)]
pub struct ApiPartialGuildChannel {
    /// The ID of the channel
    pub id: GenericChannelId,
    /// The name of the channel
    pub name: String,
    /// The position of the channel in the guild
    pub position: u16,
    /// The ID of the parent channel, if any
    pub parent_id: Option<GenericChannelId>,
    /// The type of the channel
    pub r#type: u8,
}

#[derive(Debug, Serialize, Clone)]
pub struct ApiPartialRole {
    /// The ID of the role
    pub id: RoleId,
    /// The name of the role
    pub name: String,
    /// The position of the role in the guild
    pub position: i16,
    /// Permissions of the role
    pub permissions: Permissions,
}

#[derive(Debug, Serialize)]
pub struct BaseGuildUserInfo {
    pub owner_id: String,
    pub name: String,
    pub icon: Option<String>,
    /// List of all roles in the server
    pub roles: Vec<ApiPartialRole>,
    /// List of roles the user has
    pub user_roles: Vec<RoleId>,
    /// List of roles the bot has
    pub bot_roles: Vec<RoleId>,
    /// List of all channels in the server
    pub channels: Vec<GuildChannelWithPermissions>,
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

impl IntoLua for PartialUser {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 4)?;
        table.set("id", self.id)?;
        table.set("username", self.username)?;
        table.set("global_name", self.global_name)?;
        table.set("avatar", self.avatar)?;
        Ok(LuaValue::Table(table))
    }
}