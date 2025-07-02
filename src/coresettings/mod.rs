//! Temporary core settings implementation for AntiRaid
//!
//! Will be removed once @antiraid/pages is ready for practical use

use std::sync::LazyLock;

use crate::coresettings::data::SettingsData;
pub mod data;
pub mod settings;

pub fn config_options() -> Vec<ar_settings::types::Setting<data::SettingsData>> {
    vec![
        (*settings::GUILD_TEMPLATES).clone(),
        (*settings::GUILD_TEMPLATES_KV).clone(),
        (*settings::GUILD_TEMPLATE_SHOP).clone(),
        (*settings::LOCKDOWN_SETTINGS).clone(),
    ]
}

pub fn str_to_setting(setting: &str) -> Option<&ar_settings::types::Setting<SettingsData>> {
    match setting {
        "scripts" => Some(&settings::GUILD_TEMPLATES),
        "script_kv" => Some(&settings::GUILD_TEMPLATES_KV),
        "script_shop" => Some(&settings::GUILD_TEMPLATE_SHOP),
        "lockdown_guilds" => Some(&settings::LOCKDOWN_SETTINGS),
        _ => None,
    }
}
