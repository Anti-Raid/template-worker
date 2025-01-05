use mlua::prelude::*;

pub fn plugin_docs() -> crate::doclib::Plugin {
    crate::doclib::Plugin::default()
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
        .type_mut("StaffPermissions", "StaffPermissions as per kittycat terminology.", |t| {
            t
            .example(std::sync::Arc::new(kittycat::perms::StaffPermissions {
                perm_overrides: vec![
                    kittycat::perms::Permission::from_string("~moderation.ban"),
                    kittycat::perms::Permission::from_string("~moderation.kick"),    
                ],
                user_positions: vec![
                    kittycat::perms::PartialStaffPosition {
                        id: "1234567890".to_string(),
                        index: 1,
                        perms: vec![
                            kittycat::perms::Permission::from_string("moderation.ban"),
                            kittycat::perms::Permission::from_string("moderation.kick"),
                        ]
                    },
                    kittycat::perms::PartialStaffPosition {
                        id: "0987654321".to_string(),
                        index: 2,
                        perms: vec![
                            kittycat::perms::Permission::from_string("moderation.ban"),
                            kittycat::perms::Permission::from_string("moderation.kick"),
                        ]
                    }
                ]
            }))
            .field("perm_overrides", |f| f.typ("{Permission}").description("Permission overrides on the member."))
            .field("user_positions", |f| f.typ("{PartialStaffPosition}").description("The staff positions of the user."))
        })
        .type_mut("PartialStaffPosition", "PartialStaffPosition as per kittycat terminology.", |t| {
            t
            .example(std::sync::Arc::new(kittycat::perms::PartialStaffPosition {
                id: "1234567890".to_string(),
                index: 1,
                perms: vec![
                    kittycat::perms::Permission::from_string("moderation.ban"),
                    kittycat::perms::Permission::from_string("moderation.kick"),
                ]
            }))
            .field("id", |f| f.typ("string").description("The ID of the staff member."))
            .field("index", |f| f.typ("number").description("The index of the staff member."))
            .field("perms", |f| f.typ("{Permission}").description("The permissions of the staff member."))
        })
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
        .method_mut("staff_permissions_resolve", |m| {
            m.description("Resolves a StaffPermissions object into a list of Permission objects. See https://github.com/InfinityBotList/kittycat for more details")
            .parameter("sp", |p| {
                p.typ("StaffPermissions").description("The StaffPermissions object to resolve.")
            })
            .return_("permissions", |r| {
                r.typ("{Permission}").description("The resolved list of Permission objects.")
            })
        })
        .method_mut("check_patch_changes", |m| {
            m.description("Checks if a list of permissions can be patched to another list of permissions.")
            .parameter("manager_perms", |p| {
                p.typ("{Permission}").description("The permissions of the manager.")
            })
            .parameter("current_perms", |p| {
                p.typ("{Permission}").description("The current permissions of the user.")
            })
            .parameter("new_perms", |p| {
                p.typ("{Permission}").description("The new permissions of the user.")
            })
            .return_("can_patch", |r| {
                r.typ("bool").description("Whether the permissions can be patched.")
            })
            .return_("error", |r| {
                r.typ("any").description("The error if the permissions cannot be patched. Will contain ``type`` field with the error type and additional fields depending on the error type.")
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

    module.set(
        "staff_permissions_resolve",
        lua.create_function(|lua, sp: LuaValue| {
            let sp = lua.from_value::<kittycat::perms::StaffPermissions>(sp)?;
            let resolved = sp.resolve();
            lua.to_value(&resolved)
        })?,
    )?;

    module.set(
        "check_patch_changes",
        lua.create_function(
            |lua, (manager_perms, current_perms, new_perms): (LuaValue, LuaValue, LuaValue)| {
                let manager_perms: Vec<kittycat::perms::Permission> =
                    lua.from_value(manager_perms)?;
                let current_perms: Vec<kittycat::perms::Permission> =
                    lua.from_value(current_perms)?;
                let new_perms: Vec<kittycat::perms::Permission> = lua.from_value(new_perms)?;
                let changes = kittycat::perms::check_patch_changes(
                    &manager_perms,
                    &current_perms,
                    &new_perms,
                );

                match changes {
                    Ok(()) => Ok((true, LuaValue::Nil)),
                    Err(e) => match e {
                        kittycat::perms::CheckPatchChangesError::NoPermission { permission } => {
                            Ok((
                                false,
                                lua.to_value(&serde_json::json!({
                                    "type": "NoPermission",
                                    "permission": permission
                                }))?,
                            ))
                        }
                        kittycat::perms::CheckPatchChangesError::LacksNegatorForWildcard {
                            wildcard,
                            negator,
                        } => Ok((
                            false,
                            lua.to_value(&serde_json::json!({
                                "type": "LacksNegatorForWildcard",
                                "wildcard": wildcard,
                                "negator": negator
                            }))?,
                        )),
                    },
                }
            },
        )?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
