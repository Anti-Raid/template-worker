pub mod ctxprovider;

/// Provides the config data involving kittycat permissions
pub(crate) fn kittycat_permission_config_data(
) -> silverpelt::member_permission_calc::GetKittycatPermsConfigData {
    silverpelt::member_permission_calc::GetKittycatPermsConfigData {
        main_server_id: config::CONFIG.servers.main,
        root_users: config::CONFIG.discord_auth.root_users.as_ref(),
    }
}

/// Provides the config data involving sandwich http api
pub(crate) fn sandwich_config() -> sandwich_driver::SandwichConfigData {
    sandwich_driver::SandwichConfigData {
        http_api: config::CONFIG.meta.sandwich_http_api.as_str(),
    }
}
