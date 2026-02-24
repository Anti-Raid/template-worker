use serenity::all::*;
use std::collections::HashMap;
use serde_json::Value;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommand {
    #[serde(rename = "type")]
    pub kind: Option<CommandType>,
    pub handler: Option<EntryPointHandlerType>,

    pub name: Option<String>,
    pub name_localizations: HashMap<String, String>,
    pub description: Option<String>,
    pub description_localizations: HashMap<String, String>,
    pub default_member_permissions: Option<Permissions>,
    pub dm_permission: Option<bool>,
    pub integration_types: Option<Vec<InstallationContext>>,
    pub contexts: Option<Vec<InteractionContext>>,
    pub nsfw: bool,
    pub options: Vec<CreateCommandOption>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommandOption {
    #[serde(rename = "type")]
    pub kind: CommandOptionType,
    pub name: String,
    pub name_localizations: Option<HashMap<String, String>>,
    pub description: String,
    pub description_localizations: Option<HashMap<String, String>>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub choices: Vec<CreateCommandOptionChoice>,
    #[serde(default)]
    pub options: Vec<CreateCommandOption>,
    #[serde(default)]
    pub channel_types: Vec<ChannelType>,
    #[serde(default)]
    pub min_value: Option<serde_json::Number>,
    #[serde(default)]
    pub max_value: Option<serde_json::Number>,
    #[serde(default)]
    pub min_length: Option<u16>,
    #[serde(default)]
    pub max_length: Option<u16>,
    #[serde(default)]
    pub autocomplete: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommandOptionChoice {
    pub name: String,
    pub name_localizations: Option<HashMap<String, String>>,
    pub value: Value,
}
