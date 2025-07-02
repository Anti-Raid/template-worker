//! Temporary core settings implementation for AntiRaid
//!
//! Will be removed once @antiraid/pages is ready for practical use

use std::sync::LazyLock;

use crate::coresettings::data::SettingsData;
pub mod data;
pub mod settings;

pub fn config_options() -> Vec<ar_settings::types::Setting<data::SettingsData>> {
    vec![
        (*settings::GUILD_MEMBERS).clone(),
        (*settings::GUILD_TEMPLATES).clone(),
        (*settings::GUILD_TEMPLATES_KV).clone(),
        (*settings::GUILD_TEMPLATE_SHOP).clone(),
        (*settings::GUILD_TEMPLATE_SHOP_PUBLIC_LIST).clone(),
        (*settings::LOCKDOWN_SETTINGS).clone(),
        (*settings::LOCKDOWNS).clone(),
    ]
}

pub fn str_to_setting(setting: &str) -> Option<&ar_settings::types::Setting<SettingsData>> {
    match setting {
        "guild_members" => Some(&settings::GUILD_MEMBERS),
        "scripts" => Some(&settings::GUILD_TEMPLATES),
        "script_kv" => Some(&settings::GUILD_TEMPLATES_KV),
        "script_shop" => Some(&settings::GUILD_TEMPLATE_SHOP),
        "template_shop_public_list" => Some(&settings::GUILD_TEMPLATE_SHOP_PUBLIC_LIST),
        "lockdown_guilds" => Some(&settings::LOCKDOWN_SETTINGS),
        "lockdowns" => Some(&settings::LOCKDOWNS),
        _ => None,
    }
}
