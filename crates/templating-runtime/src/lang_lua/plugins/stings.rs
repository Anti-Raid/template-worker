use crate::lang_lua::{primitives::TemplateContextRef, state};
use mlua::prelude::*;
use std::rc::Rc;

use super::promise::lua_promise;

pub fn plugin_docs() -> crate::doclib::Plugin {
    crate::doclib::Plugin::default()
        .name("@antiraid/stings")
        .description("List, get, create, update and delete stings on Anti-Raid.")
        .enum_mut("StingTarget", "The target of the sting.", |e| {
            e
                .variant("system", |v| {
                    v.description("A system-target (no associated user)")
                })
                .variant("user:{user_id}", |v| {
                    v.description("A user-target")
                })
        })
        .type_mut(
            "StingCreate",
            "A type representing a new sting to be created.",
            |t| {
                t
                .example(std::sync::Arc::new(silverpelt::stings::StingCreate {
                    src: Some("test".to_string()),
                    stings: 10,
                    reason: Some("test".to_string()),
                    void_reason: None,
                    guild_id: serenity::all::GuildId::new(128384),
                    creator: silverpelt::stings::StingTarget::System,
                    target: silverpelt::stings::StingTarget::User(serenity::all::UserId::new(1945824)),
                    state: silverpelt::stings::StingState::Active,
                    duration: Some(std::time::Duration::from_secs(60)),
                    sting_data: Some(serde_json::json!({"a": "b"})),
                }))
                .field("src", |f| {
                    f.typ("string?").description("The source of the sting.")
                })
                .field("stings", |f| {
                    f.typ("number").description("The number of stings.")
                })
                .field("reason", |f| {
                    f.typ("string?").description("The reason for the stings.")
                })
                .field("void_reason", |f| {
                    f.typ("string?")
                        .description("The reason the stings were voided.")
                })
                .field("guild_id", |f| {
                    f.typ("string")
                        .description("The guild ID the sting targets. **MUST MATCH THE GUILD ID THE TEMPLATE IS RUNNING ON**")
                })
                .field("creator", |f| {
                    f.typ("StingTarget")
                        .description("The creator of the sting.")
                })
                .field("target", |f| {
                    f.typ("StingTarget").description("The target of the sting.")
                })
                .field("state", |f| {
                    f.typ("string").description("The state of the sting. Must be one of 'active', 'voided' or 'handled'")
                })
                .field("duration", |f| {
                    f.typ("Duration?")
                        .description("When the sting expires as a duration.")
                })
                .field("sting_data", |f| {
                    f.typ("any?")
                        .description("The data/metadata present within the sting, if any.")
                })
            },
        )
        .type_mut("Sting", "Represents a sting on AntiRaid", |t| {
            t
                .example(std::sync::Arc::new(silverpelt::stings::Sting {
                    id: sqlx::types::Uuid::parse_str("470a2958-3827-4e59-8b97-928a583a37a3").unwrap(),
                    src: Some("test".to_string()),
                    stings: 10,
                    reason: Some("test".to_string()),
                    void_reason: None,
                    guild_id: serenity::all::GuildId::new(128384),
                    creator: silverpelt::stings::StingTarget::System,
                    target: silverpelt::stings::StingTarget::User(serenity::all::UserId::new(1945824)),
                    state: silverpelt::stings::StingState::Active,
                    created_at: chrono::Utc::now(),
                    duration: Some(std::time::Duration::from_secs(60)),
                    sting_data: Some(serde_json::json!({"a": "b"})),
                    handle_log: serde_json::json!({"a": "b"}),
                }))
                .field("id", |f| {
                    f.typ("string").description("The sting ID.")
                })
                .field("src", |f| {
                    f.typ("string?").description("The source of the sting.")
                })
                .field("stings", |f| {
                    f.typ("number").description("The number of stings.")
                })
                .field("reason", |f| {
                    f.typ("string?").description("The reason for the stings.")
                })
                .field("void_reason", |f| {
                    f.typ("string?")
                        .description("The reason the stings were voided.")
                })
                .field("guild_id", |f| {
                    f.typ("string")
                        .description("The guild ID the sting targets. **MUST MATCH THE GUILD ID THE TEMPLATE IS RUNNING ON**")
                })
                .field("creator", |f| {
                    f.typ("StingTarget")
                        .description("The creator of the sting.")
                })
                .field("target", |f| {
                    f.typ("StingTarget").description("The target of the sting.")
                })
                .field("state", |f| {
                    f.typ("StingState").description("The state of the sting.")
                })
                .field("duration", |f| {
                    f.typ("Duration?")
                        .description("When the sting expires as a duration.")
                })
                .field("sting_data", |f| {
                    f.typ("any?")
                        .description("The data/metadata present within the sting, if any.")
                })
                .field("handle_log", |f| {
                    f.typ("any").description("The handle log encountered while handling the sting.")
                })
                .field("created_at", |f| {
                    f.typ("string")
                        .description("When the sting was created at.")
                })
        })
        .type_mut("StingExecutor", "An sting executor is used to execute actions related to stings from Lua templates", |mut t| {
            t.method_mut("list", |mut m| {
                m
                .parameter("page", |p| {
                    p.typ("number").description("The page number to fetch.")
                })
                .return_("stings", |r| {
                    r.typ("{Sting}").description("The list of stings.")
                })
                .is_promise(true)
            })
            .method_mut("get", |mut m| {
                m
                .parameter("id", |p| {
                    p.typ("string").description("The sting ID.")
                })
                .return_("sting", |r| {
                    r.typ("Sting").description("The sting.")
                })
                .is_promise(true)
            })
            .method_mut("create", |mut m| {
                m
                .parameter("data", |p| {
                    p.typ("StingCreate").description("The sting data.")
                })
                .return_("id", |r| {
                    r.typ("string").description("The sting ID of the created sting.")
                })
                .is_promise(true)
            })
            .method_mut("update", |mut m| {
                m
                .parameter("data", |p| {
                    p.typ("Sting").description("The sting to update to. Note that if an invalid ID is used, this method may either do nothing or error out.")
                })
                .is_promise(true)
            })
            .method_mut("delete", |mut m| {
                m
                .parameter("id", |p| {
                    p.typ("string").description("The sting ID.")
                })
                .is_promise(true)
            })
        })
}

/// An sting executor is used to execute actions related to stings from Lua
/// templates
#[derive(Clone)]
pub struct StingExecutor {
    pragma: crate::TemplatePragma,
    guild_id: serenity::all::GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    ratelimits: Rc<state::LuaRatelimits>,
}

impl StingExecutor {
    pub fn check_action(&self, action: String) -> Result<(), crate::Error> {
        if !self
            .pragma
            .allowed_caps
            .contains(&format!("sting:{}", action))
        {
            return Err("Sting operation not allowed in this template context".into());
        }

        self.ratelimits.check(&action)?;

        Ok(())
    }
}

impl LuaUserData for StingExecutor {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("list", |_, this, page: usize| {
            Ok(lua_promise!(this, page, |lua, this, page|, {
                this.check_action("list".to_string())
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                let stings = silverpelt::stings::Sting::list(&this.pool, this.guild_id, page)
                    .await
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                let v = lua.to_value(&stings)?;

                Ok(v)
            }))
        });

        methods.add_method("get", |_, this, id: String| {
            let id = sqlx::types::Uuid::parse_str(&id).map_err(|e| {
                LuaError::FromLuaConversionError {
                    from: "string",
                    to: "uuid".to_string(),
                    message: Some(e.to_string()),
                }
            })?;

            Ok(lua_promise!(this, id, |lua, this, id|, {
                this.check_action("get".to_string())
                .map_err(|e| LuaError::runtime(e.to_string()))?;

                let sting = silverpelt::stings::Sting::get(&this.pool, this.guild_id, id)
                    .await
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                let v = lua.to_value(&sting)?;

                Ok(v)
            }))
        });

        methods.add_method("create", |_, this, data: LuaValue| {
            Ok(lua_promise!(this, data, |lua, this, data|, {
                let sting = lua.from_value::<silverpelt::stings::StingCreate>(data)?;

                this.check_action("create".to_string())
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                if sting.guild_id != this.guild_id {
                    return Err(LuaError::external("Guild ID mismatch"));
                }

                let sting = sting
                    .create_and_dispatch_returning_id(this.serenity_context.clone(), &this.pool)
                    .await
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                Ok(sting.to_string())
            }))
        });

        methods.add_method("update", |_, this, data: LuaValue| {
            Ok(lua_promise!(this, data, |lua, this, data|, {
                let sting = lua.from_value::<silverpelt::stings::Sting>(data)?;

                this.check_action("update".to_string())
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                if sting.guild_id != this.guild_id {
                    return Err(LuaError::external("Guild ID mismatch"));
                }

                sting
                    .update_and_dispatch(&this.pool, this.serenity_context.clone())
                    .await
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                Ok(())
            }))
        });

        methods.add_method("delete", |lua, this, id: LuaValue| {
            let id = lua.from_value::<sqlx::types::Uuid>(id).map_err(|e| {
                LuaError::FromLuaConversionError {
                    from: "string",
                    to: "uuid".to_string(),
                    message: Some(e.to_string()),
                }
            })?;

            Ok(lua_promise!(this, id, |_lua, this, id|, {
                this.check_action("delete".to_string())
                .map_err(|e| LuaError::runtime(e.to_string()))?;

                let Some(sting) = silverpelt::stings::Sting::get(&this.pool, this.guild_id, id)
                    .await
                    .map_err(|e| LuaError::runtime(e.to_string()))?
                else {
                    return Err(LuaError::external("Sting not found"));
                };

                sting
                    .delete_and_dispatch(&this.pool, this.serenity_context.clone())
                    .await
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                Ok(())
            }))
        });
    }
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "new",
        lua.create_function(|_, (token,): (TemplateContextRef,)| {
            let executor = StingExecutor {
                pragma: token.template_data.pragma.clone(),
                guild_id: token.guild_state.guild_id,
                serenity_context: token.guild_state.serenity_context.clone(),
                ratelimits: token.guild_state.sting_ratelimits.clone(),
                pool: token.guild_state.pool.clone(),
            };

            Ok(executor)
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
