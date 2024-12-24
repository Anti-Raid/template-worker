use mlua::prelude::*;

pub fn plugin_docs() -> templating_docgen::Plugin {
    templating_docgen::Plugin::default()
        .name("@antiraid/permissions")
        .description("Utilities for handling permission checks.")
        .type_mut(
            "Permission",
            "Permission is the primitive permission type used by AntiRaid. See https://github.com/InfinityBotList/kittycat for more information",
            |t| {
                t
                .example(std::sync::Arc::new(kittycat::perms::Permission::from_string("moderation.ban")))
                .field("namespace", |f| f.typ("string").description("The namespace of the permission."))
                .field("perm", |f| f.typ("string").description("The permission bit on the namespace."))
                .field("negator", |f| f.typ("bool").description("Whether the permission is a negator permission or not"))
            },
        )
        .method_mut("permission_from_string", |m| {
            m.description("Returns a Permission object from a string.")
            .parameter("perm_string", |p| {
                p.typ("string").description("The string to parse into a Permission object.")
            })
            .return_("permission", |r| {
                r.typ("Permission").description("The parsed Permission object.")
            })
        })
        .method_mut("permission_to_string", |m| {
            m.description("Returns a string from a Permission object.")
            .parameter("permission", |p| {
                p.typ("Permission").description("The Permission object to parse into a string.")
            })
            .return_("perm_string", |r| {
                r.typ("string").description("The parsed string.")
            })
        })
        .method_mut("has_perm", |m| {
            m.description("Checks if a list of permissions in Permission object form contains a specific permission.")
            .parameter("permissions", |p| {
                p.typ("{Permission}").description("The list of permissions")
            })
            .parameter("permission", |p| {
                p.typ("Permission").description("The permission to check for.")
            })
            .return_("has_perm", |r| {
                r.typ("bool").description("Whether the permission is present in the list of permissions as per kittycat rules.")
            })
        })
        .method_mut("has_perm_str", |m| {
            m.description("Checks if a list of permissions in canonical string form contains a specific permission.")
            .parameter("permissions", |p| {
                p.typ("{string}").description("The list of permissions")
            })
            .parameter("permission", |p| {
                p.typ("string").description("The permission to check for.")
            })
            .return_("has_perm", |r| {
                r.typ("bool").description("Whether the permission is present in the list of permissions as per kittycat rules.")
            })
        })
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "permission_from_string",
        lua.create_function(|lua, (perm_string,): (String,)| {
            let ps = kittycat::perms::Permission::from_string(&perm_string);
            lua.to_value(&ps)
        })?,
    )?;

    module.set(
        "permission_to_string",
        lua.create_function(|lua, (permission,): (LuaValue,)| {
            let perm: kittycat::perms::Permission = lua.from_value(permission)?;
            Ok(perm.to_string())
        })?,
    )?;

    module.set(
        "has_perm",
        lua.create_function(|lua, (permissions, permission): (LuaValue, LuaValue)| {
            let perm: kittycat::perms::Permission = lua.from_value(permission)?;
            let perms: Vec<kittycat::perms::Permission> = lua.from_value(permissions)?;
            Ok(kittycat::perms::has_perm(&perms, &perm))
        })?,
    )?;

    module.set(
        "has_perm_str",
        lua.create_function(|_, (permissions, permission): (Vec<String>, String)| {
            Ok(kittycat::perms::has_perm_str(&permissions, &permission))
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
