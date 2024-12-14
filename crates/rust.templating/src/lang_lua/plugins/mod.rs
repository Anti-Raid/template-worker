pub mod discord;
pub mod img_captcha;
pub mod interop;
pub mod kv;
pub mod lune;
pub mod page;
pub mod permissions;
pub mod promise;
pub mod stings;
pub mod typesext;

use mlua::prelude::*;
use std::sync::LazyLock;

pub static PLUGINS: LazyLock<indexmap::IndexMap<String, (ModuleFn, Option<ModuleDocFn>)>> =
    LazyLock::new(|| {
        indexmap::indexmap! {
            "@antiraid/discord".to_string() => (discord::init_plugin as ModuleFn, Some(discord::plugin_docs as ModuleDocFn)),
            "@antiraid/interop".to_string() => (interop::init_plugin as ModuleFn, Some(interop::plugin_docs as ModuleDocFn)),
            "@antiraid/img_captcha".to_string() => (img_captcha::init_plugin as ModuleFn, Some(img_captcha::plugin_docs as ModuleDocFn)),
            "@antiraid/kv".to_string() => (kv::init_plugin as ModuleFn, Some(kv::plugin_docs as ModuleDocFn)),
            "@antiraid/page".to_string() => (page::init_plugin as ModuleFn, Some(page::plugin_docs as ModuleDocFn)),
            "@antiraid/permissions".to_string() => (permissions::init_plugin as ModuleFn, Some(permissions::plugin_docs as ModuleDocFn)),
            "@antiraid/promise".to_string() => (promise::init_plugin as ModuleFn, None),
            "@antiraid/stings".to_string() => (stings::init_plugin as ModuleFn, Some(stings::plugin_docs as ModuleDocFn)),
            "@antiraid/typesext".to_string() => (typesext::init_plugin as ModuleFn, Some(typesext::plugin_docs as ModuleDocFn)),

            // External plugins
            "@lune/datetime".to_string() => (lune::datetime::init_plugin as ModuleFn, None as Option<ModuleDocFn>),
            "@lune/regex".to_string() => (lune::regex::init_plugin as ModuleFn, None as Option<ModuleDocFn>),
            "@lune/serde".to_string() => (lune::serde::init_plugin as ModuleFn, None as Option<ModuleDocFn>),
            "@lune/roblox".to_string() => (lune::roblox::init_plugin as ModuleFn, None as Option<ModuleDocFn>),
        }
    });

type ModuleFn = fn(&Lua) -> LuaResult<LuaTable>;
type ModuleDocFn = fn() -> templating_docgen::Plugin;

pub fn require(lua: &Lua, plugin_name: String) -> LuaResult<LuaTable> {
    match PLUGINS.get(plugin_name.as_str()) {
        Some(plugin) => {
            // Get table from vm cache
            if let Ok(table) = lua.named_registry_value::<LuaTable>(&plugin_name) {
                return Ok(table);
            }

            let res = plugin.0(lua);

            if let Ok(table) = &res {
                lua.set_named_registry_value(&plugin_name, table.clone())?;
            }

            res
        }
        None => {
            if let Ok(table) = lua.globals().get::<LuaTable>(plugin_name.clone()) {
                return Ok(table);
            }

            Err(LuaError::runtime(format!(
                "module '{}' not found",
                plugin_name
            )))
        }
    }
}
