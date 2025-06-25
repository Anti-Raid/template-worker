//! Temporary core settings implementation for AntiRaid
//!
//! Will be removed once @antiraid/pages is ready for practical use
pub mod data;
pub mod settings;

pub fn config_options() -> Vec<ar_settings::types::Setting<data::SettingsData>> {
    vec![
        (*settings::GUILD_ROLES).clone(),
        (*settings::GUILD_MEMBERS).clone(),
        (*settings::GUILD_TEMPLATES).clone(),
        (*settings::GUILD_TEMPLATES_KV).clone(),
        (*settings::GUILD_TEMPLATE_SHOP).clone(),
        (*settings::GUILD_TEMPLATE_SHOP_PUBLIC_LIST).clone(),
        (*settings::LOCKDOWN_SETTINGS).clone(),
        (*settings::LOCKDOWNS).clone(),
    ]
}
