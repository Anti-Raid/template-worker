use super::promise::lua_promise;
use crate::lang_lua::{primitives::TemplateContextRef, state};
use mlua::prelude::*;
use std::{rc::Rc, str::FromStr};

#[derive(Clone)]
/// An lockdown executor is used to manage AntiRaid lockdowns from Lua
/// templates
pub struct LockdownExecutor {
    pragma: crate::TemplatePragma,
    guild_id: serenity::all::GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
    ratelimits: Rc<state::Ratelimits>,
}

// @userdata LockdownExecutor
//
// Executes actions on discord
impl LockdownExecutor {
    pub fn check_action(&self, action: String) -> LuaResult<()> {
        if !self
            .pragma
            .allowed_caps
            .contains(&format!("lockdown:{}", action))
        {
            return Err(LuaError::runtime(
                "Lockdown action is not allowed in this template context",
            ));
        }

        self.ratelimits
            .lockdowns
            .check(&action)
            .map_err(|e| LuaError::external(e.to_string()))?;

        Ok(())
    }
}

pub fn plugin_docs() -> crate::doclib::Plugin {
    crate::doclib::Plugin::default()
        .name("@antiraid/lockdowns")
        .description("This plugin allows for templates to interact with AntiRaid lockdowns")
        .type_mut("Lockdown", "A created lockdown", |t| {
            t.example(std::sync::Arc::new(lockdowns::Lockdown {
                id: sqlx::types::Uuid::from_str("805c0dd1-a625-4875-81e4-8edc6a14f659")
                    .expect("Failed to parse UUID"),
                reason: "Testing".to_string(),
                r#type: Box::new(lockdowns::qsl::QuickServerLockdown {}),
                data: serde_json::json!({}),
                created_at: chrono::Utc::now(),
            }))
            .field("id", |f| {
                f.typ("string").description("The id of the lockdown")
            })
            .field("reason", |f| {
                f.typ("string").description("The reason for the lockdown")
            })
            .field("type", |f| {
                f.typ("string")
                    .description("The type of lockdown in string form")
            })
            .field("data", |f| {
                f.typ("any")
                    .description("The data associated with the lockdown")
            })
            .field("created_at", |f| {
                f.typ("string")
                    .description("The time the lockdown was created")
            })
        })
        .type_mut(
            "LockdownExecutor",
            "An executor for listing, creating and removing lockdowns",
            |mut t| {
                t.method_mut("list", |m| {
                    m.is_promise(true)
                        .description("Lists all active lockdowns")
                        .return_("lockdowns", |r| {
                            r.typ("{Lockdown}")
                                .description("A list of all currently active lockdowns")
                        })
                })
                .method_mut("qsl", |m| {
                    m.is_promise(true)
                        .description("Starts a quick server lockdown")
                        .parameter("reason", |p| {
                            p.description("The reason for the lockdown").typ("string")
                        })
                })
                .method_mut("tsl", |m| {
                    m.is_promise(true)
                        .description("Starts a traditional server lockdown")
                        .parameter("reason", |p| {
                            p.description("The reason for the lockdown").typ("string")
                        })
                })
                .method_mut("scl", |m| {
                    m.is_promise(true)
                        .description("Starts a lockdown on a single channel")
                        .parameter("channel", |p| {
                            p.description("The channel to lock down").typ("string")
                        })
                        .parameter("reason", |p| {
                            p.description("The reason for the lockdown").typ("string")
                        })
                })
                .method_mut("role", |m| {
                    m.is_promise(true)
                        .description("Starts a lockdown on a role")
                        .parameter("role", |p| {
                            p.description("The role to lock down").typ("string")
                        })
                        .parameter("reason", |p| {
                            p.description("The reason for the lockdown").typ("string")
                        })
                })
                .method_mut("remove", |m| {
                    m.is_promise(true)
                        .description("Removes a lockdown")
                        .parameter("id", |p| {
                            p.description("The id of the lockdown to remove")
                                .typ("string")
                        })
                })
            },
        )
        .method_mut("new", |mut m| {
            m.parameter("token", |p| {
                p.description("The token of the template to use")
                    .typ("TemplateContext")
            })
            .return_("executor", |r| {
                r.description("A lockdown executor").typ("LockdownExecutor")
            })
        })
}

impl LuaUserData for LockdownExecutor {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("list", |_, this, _g: ()| {
            Ok(lua_promise!(this, _g, |lua, this, _g|, {
                this.check_action("list".to_string())?;

                // Get the current lockdown set
                let lockdowns = lockdowns::LockdownSet::guild(this.guild_id, &this.pool)
                    .await
                    .map_err(|e| format!("Error while fetching lockdown set: {}", e))
                    .map_err(LuaError::external)?;

                let lockdowns = lua.to_value(&lockdowns.lockdowns);

                Ok(lockdowns)
            }))
        });

        methods.add_method("qsl", |_, this, reason: String| {
            Ok(lua_promise!(this, reason, |_lua, this, reason|, {
                this.check_action("qsl".to_string())?;

                // Get the current lockdown set
                let mut lockdowns = lockdowns::LockdownSet::guild(this.guild_id, &this.pool)
                    .await
                    .map_err(|e| format!("Error while fetching lockdown set: {}", e))
                    .map_err(LuaError::external)?;

                // Create the lockdown
                let lockdown_type = lockdowns::qsl::QuickServerLockdown {};

                let lockdown_data = lockdowns::LockdownData {
                    cache: &this.serenity_context.cache,
                    http: &this.serenity_context.http,
                    pool: this.pool.clone(),
                    reqwest: this.reqwest_client.clone(),
                };

                lockdowns
                    .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
                    .await
                    .map_err(|e| format!("Error while applying lockdown: {}", e))
                    .map_err(LuaError::external)?;

                Ok(())
            }))
        });

        methods.add_method("qsl", |_, this, reason: String| {
            Ok(lua_promise!(this, reason, |_lua, this, reason|, {
                this.check_action("qsl".to_string())?;

                // Get the current lockdown set
                let mut lockdowns = lockdowns::LockdownSet::guild(this.guild_id, &this.pool)
                    .await
                    .map_err(|e| format!("Error while fetching lockdown set: {}", e))
                    .map_err(LuaError::external)?;

                // Create the lockdown
                let lockdown_type = lockdowns::qsl::QuickServerLockdown {};

                let lockdown_data = lockdowns::LockdownData {
                    cache: &this.serenity_context.cache,
                    http: &this.serenity_context.http,
                    pool: this.pool.clone(),
                    reqwest: this.reqwest_client.clone(),
                };

                lockdowns
                    .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
                    .await
                    .map_err(|e| format!("Error while applying lockdown: {}", e))
                    .map_err(LuaError::external)?;

                Ok(())
            }))
        });

        methods.add_method("tsl", |_, this, reason: String| {
            Ok(lua_promise!(this, reason, |_lua, this, reason|, {
                this.check_action("tsl".to_string())?;

                // Get the current lockdown set
                let mut lockdowns = lockdowns::LockdownSet::guild(this.guild_id, &this.pool)
                    .await
                    .map_err(|e| format!("Error while fetching lockdown set: {}", e))
                    .map_err(LuaError::external)?;

                // Create the lockdown
                let lockdown_type = lockdowns::tsl::TraditionalServerLockdown {};

                let lockdown_data = lockdowns::LockdownData {
                    cache: &this.serenity_context.cache,
                    http: &this.serenity_context.http,
                    pool: this.pool.clone(),
                    reqwest: this.reqwest_client.clone(),
                };

                lockdowns
                    .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
                    .await
                    .map_err(|e| format!("Error while applying lockdown: {}", e))
                    .map_err(LuaError::external)?;

                Ok(())
            }))
        });

        methods.add_method("scl", |_, this, (channel, reason): (String, String)| {
            let channel: serenity::all::ChannelId = channel.parse().map_err(|e| {
                LuaError::external(format!("Error while parsing channel id: {}", e))
            })?;

            Ok(
                lua_promise!(this, channel, reason, |_lua, this, channel, reason|, {
                    this.check_action("scl".to_string())?;

                    // Get the current lockdown set
                    let mut lockdowns = lockdowns::LockdownSet::guild(this.guild_id, &this.pool)
                        .await
                        .map_err(|e| format!("Error while fetching lockdown set: {}", e))
                        .map_err(LuaError::external)?;

                    // Create the lockdown
                    let lockdown_type = lockdowns::scl::SingleChannelLockdown(channel);

                    let lockdown_data = lockdowns::LockdownData {
                        cache: &this.serenity_context.cache,
                        http: &this.serenity_context.http,
                        pool: this.pool.clone(),
                        reqwest: this.reqwest_client.clone(),
                    };

                    lockdowns
                        .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
                        .await
                        .map_err(|e| format!("Error while applying lockdown: {}", e))
                        .map_err(LuaError::external)?;

                    Ok(())
                }),
            )
        });

        methods.add_method("role", |_, this, (role, reason): (String, String)| {
            let role: serenity::all::RoleId = role
                .parse()
                .map_err(|e| LuaError::external(format!("Error while parsing role id: {}", e)))?;

            Ok(
                lua_promise!(this, role, reason, |_lua, this, role, reason|, {
                    this.check_action("role".to_string())?;

                    // Get the current lockdown set
                    let mut lockdowns = lockdowns::LockdownSet::guild(this.guild_id, &this.pool)
                        .await
                        .map_err(|e| format!("Error while fetching lockdown set: {}", e))
                        .map_err(LuaError::external)?;

                    // Create the lockdown
                    let lockdown_type = lockdowns::role::RoleLockdown(role);

                    let lockdown_data = lockdowns::LockdownData {
                        cache: &this.serenity_context.cache,
                        http: &this.serenity_context.http,
                        pool: this.pool.clone(),
                        reqwest: this.reqwest_client.clone(),
                    };

                    lockdowns
                        .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
                        .await
                        .map_err(|e| format!("Error while applying lockdown: {}", e))
                        .map_err(LuaError::external)?;

                    Ok(())
                }),
            )
        });

        methods.add_method("remove", |_, this, id: String| {
            let id: sqlx::types::Uuid = id.parse().map_err(|e| {
                LuaError::external(format!("Error while parsing lockdown id: {}", e))
            })?;

            Ok(lua_promise!(this, id, |_lua, this, id|, {
                this.check_action("remove".to_string())?;

                // Get the current lockdown set
                let mut lockdowns = lockdowns::LockdownSet::guild(this.guild_id, &this.pool)
                    .await
                    .map_err(|e| format!("Error while fetching lockdown set: {}", e))
                    .map_err(LuaError::external)?;

                let lockdown_data = lockdowns::LockdownData {
                    cache: &this.serenity_context.cache,
                    http: &this.serenity_context.http,
                    pool: this.pool.clone(),
                    reqwest: this.reqwest_client.clone(),
                };

                lockdowns
                    .easy_remove(id, &lockdown_data)
                    .await
                    .map_err(|e| format!("Error while applying lockdown: {}", e))
                    .map_err(LuaError::external)?;

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
            let executor = LockdownExecutor {
                pragma: token.template_data.pragma.clone(),
                guild_id: token.guild_state.guild_id,
                serenity_context: token.guild_state.serenity_context.clone(),
                reqwest_client: token.guild_state.reqwest_client.clone(),
                pool: token.guild_state.pool.clone(),
                ratelimits: token.guild_state.ratelimits.clone(),
            };

            Ok(executor)
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
