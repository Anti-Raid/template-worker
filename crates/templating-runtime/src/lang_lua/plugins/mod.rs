pub mod antiraid;
pub(crate) mod executor;
pub mod lune;

use mlua::prelude::*;
use std::sync::LazyLock;

pub static PLUGINS: LazyLock<indexmap::IndexMap<String, (ModuleFn, Option<ModuleDocFn>)>> =
    LazyLock::new(|| {
        indexmap::indexmap! {
            "@antiraid/discord".to_string() => (antiraid::discord::init_plugin as ModuleFn, Some(antiraid::discord::plugin_docs as ModuleDocFn)),
            "@antiraid/interop".to_string() => (antiraid::interop::init_plugin as ModuleFn, Some(antiraid::interop::plugin_docs as ModuleDocFn)),
            "@antiraid/img_captcha".to_string() => (antiraid::img_captcha::init_plugin as ModuleFn, Some(antiraid::img_captcha::plugin_docs as ModuleDocFn)),
            "@antiraid/kv".to_string() => (antiraid::kv::init_plugin as ModuleFn, Some(antiraid::kv::plugin_docs as ModuleDocFn)),
            "@antiraid/lazy".to_string() => (antiraid::lazy::init_plugin as ModuleFn, Some(antiraid::lazy::plugin_docs as ModuleDocFn)),
            "@antiraid/lockdowns".to_string() => (antiraid::lockdowns::init_plugin as ModuleFn, Some(antiraid::lockdowns::plugin_docs as ModuleDocFn)),
            "@antiraid/page".to_string() => (antiraid::page::init_plugin as ModuleFn, Some(antiraid::page::plugin_docs as ModuleDocFn)),
            "@antiraid/permissions".to_string() => (antiraid::permissions::init_plugin as ModuleFn, Some(antiraid::permissions::plugin_docs as ModuleDocFn)),
            "@antiraid/promise".to_string() => (antiraid::promise::init_plugin as ModuleFn, Some(antiraid::promise::plugin_docs as ModuleDocFn)),
            "@antiraid/stings".to_string() => (antiraid::stings::init_plugin as ModuleFn, Some(antiraid::stings::plugin_docs as ModuleDocFn)),
            "@antiraid/typesext".to_string() => (antiraid::typesext::init_plugin as ModuleFn, Some(antiraid::typesext::plugin_docs as ModuleDocFn)),
            "@antiraid/userinfo".to_string() => (antiraid::userinfo::init_plugin as ModuleFn, Some(antiraid::userinfo::plugin_docs as ModuleDocFn)),

            // External plugins
            "@lune/datetime".to_string() => (lune::datetime::init_plugin as ModuleFn, None as Option<ModuleDocFn>),
            "@lune/regex".to_string() => (lune::regex::init_plugin as ModuleFn, None as Option<ModuleDocFn>),
            "@lune/serde".to_string() => (lune::serde::init_plugin as ModuleFn, None as Option<ModuleDocFn>),
            "@lune/roblox".to_string() => (lune::roblox::init_plugin as ModuleFn, None as Option<ModuleDocFn>),
        }
    });

type ModuleFn = fn(&Lua) -> LuaResult<LuaTable>;
type ModuleDocFn = fn() -> crate::doclib::Plugin;

pub fn require(lua: &Lua, plugin_name: String) -> LuaResult<LuaTable> {
    if let Ok(table) = lua.globals().get::<LuaTable>(plugin_name.clone()) {
        return Ok(table);
    }

    match PLUGINS.get(plugin_name.as_str()) {
        Some(plugin) => {
            let res = plugin.0(lua);

            if let Ok(table) = &res {
                lua.set_named_registry_value(&plugin_name, table.clone())?;
            }

            res
        }
        None => Err(LuaError::runtime(format!(
            "module '{}' not found",
            plugin_name
        ))),
    }
}
