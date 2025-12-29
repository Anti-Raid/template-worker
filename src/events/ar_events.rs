use serde_json::Value;
use strum::{IntoStaticStr, VariantNames};
use ts_rs::TS;

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct GetSettingsEvent {
    #[schema(value_type = String)]
    #[ts(as = "String")]
    pub author: serenity::all::UserId,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct SettingExecuteEvent {
    /// The ID of the setting being executed
    pub id: String,
    /// The author of the event
    #[schema(value_type = String)]
    #[ts(as = "String")]
    pub author: serenity::all::UserId,
    /// The operation being performed on the setting
    pub op: String,
    /// The fields of the operation. May be a map or list of fields
    pub fields: Value,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct KeyExpiryEvent {
    pub id: String,
    pub key: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, utoipa::ToSchema, TS)]
#[ts(export)]
pub struct StartupEvent {
    pub reason: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, IntoStaticStr, VariantNames, utoipa::ToSchema, TS)]
#[must_use]
#[ts(export)]
pub enum AntiraidEvent {
    /// Fired when a key expires within the key-value store
    KeyExpiry(KeyExpiryEvent),

    /// Fired when a key is resumed
    /// 
    /// This occurs if a resumable key is set and the template is reloaded or the worker process restarted
    OnStartup(StartupEvent),

    /// A GetSettings event. Fired when settings are requested by the user
    ///
    /// E.g. when user opens dashboard etc
    GetSettings(GetSettingsEvent),

    /// A ExecuteSetting event. Fired when a setting is executed by the user
    ExecuteSetting(SettingExecuteEvent),
}

impl std::fmt::Display for AntiraidEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s: &'static str = self.into();
        write!(f, "{}", s)
    }
}

impl AntiraidEvent {
    /// Returns the variant names
    pub fn variant_names() -> &'static [&'static str] {
        Self::VARIANTS
    }

    /// Convert the event's inner data to a JSON value
    pub fn to_value(&self) -> Result<serde_json::Value, serde_json::Error> {
        match self {
            AntiraidEvent::KeyExpiry(data) => serde_json::to_value(data),
            AntiraidEvent::OnStartup(templates) => serde_json::to_value(templates),
            AntiraidEvent::GetSettings(data) => serde_json::to_value(data),
            AntiraidEvent::ExecuteSetting(data) => serde_json::to_value(data),
        }
    }
}
