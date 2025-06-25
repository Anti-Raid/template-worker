use silverpelt::data::Data;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum RequestScope {
    Guild((serenity::all::GuildId, serenity::all::UserId)),
    Anonymous,
}

impl RequestScope {
    pub fn guild_id(&self) -> Result<serenity::all::GuildId, crate::Error> {
        match self {
            RequestScope::Guild((guild_id, _)) => Ok(*guild_id),
            RequestScope::Anonymous => {
                Err("This setting cannot be used in an anonymous context".into())
            }
        }
    }

    pub fn user_id(&self) -> Result<serenity::all::UserId, crate::Error> {
        match self {
            RequestScope::Guild((_, user_id)) => Ok(*user_id),
            RequestScope::Anonymous => {
                Err("This setting cannot be used in an anonymous context".into())
            }
        }
    }
}

#[derive(Clone)]
pub struct SettingsData {
    pub data: Arc<Data>,
    pub serenity_context: serenity::all::Context,
    pub scope: RequestScope,
}

impl Default for SettingsData {
    fn default() -> Self {
        unreachable!("SettingsData::default() should never be called")
    }
}

impl serde::Serialize for SettingsData {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_unit()
    }
}

/// Given the Data and a cache_http, returns the settings data
pub fn settings_data(
    serenity_context: serenity::all::Context,
    scope: RequestScope,
) -> SettingsData {
    SettingsData {
        data: serenity_context.data::<Data>(),
        serenity_context,
        scope,
    }
}
