use crate::lang_lua::{primitives::TemplateContextRef, state};
use mlua::prelude::*;
use serde::{Deserialize, Serialize};
use std::{num::TryFromIntError, rc::Rc};

use super::promise::lua_promise;
use crate::lang_lua::plugins::executor::ExecutorScope;

/// An kv executor is used to execute key-value ops from Lua
/// templates
#[derive(Clone)]
pub struct KvExecutor {
    allowed_caps: Vec<String>,
    /// The guild ID to execute the operation on
    /// 
    /// This can be either ThisGuild or OwnerGuild
    guild_id: serenity::all::GuildId,
    /// The origin guild id
    origin_guild_id: serenity::all::GuildId,
    scope: ExecutorScope,
    pool: sqlx::PgPool,
    kv_constraints: state::LuaKVConstraints,
    ratelimits: Rc<state::Ratelimits>,
}

/// Represents a full record complete with metadata
#[derive(Serialize, Deserialize)]
pub struct KvRecord {
    pub key: String,
    pub value: serde_json::Value,
    pub exists: bool,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl KvRecord {
    fn default() -> KvRecord {
        KvRecord {
            key: "".to_string(),
            value: serde_json::Value::Null,
            exists: false,
            created_at: None,
            last_updated_at: None,
        }
    }
}

impl KvExecutor {
    pub fn check(&self, action: String, key: String) -> Result<(), crate::Error> {
        if !self
        .allowed_caps
        .contains(&"kv:*".to_string()) // KV:* means all KV operations are allowed
        && !self
        .allowed_caps
        .contains(&format!("kv:{}:*", action)) // kv:{action}:* means that the action can be performed on any key
        && !self
        .allowed_caps
        .contains(&format!("kv:{}:{}", action, key)) // kv:{action}:{key} means that the action can only be performed on said key
        && !self
        .allowed_caps
        .contains(&format!("kv:*:{}", key))  // kv:*:{key} means that any action can be performed on said key
        {
            return Err(format!("KV operation `{}` not allowed in this template context for key '{}'", action, key).into());
        }

        self.ratelimits.kv.check(&action)?; // Check rate limits

        Ok(())
    }
}

pub fn plugin_docs() -> crate::doclib::Plugin {
    crate::doclib::Plugin::default()
        .name("@antiraid/kv")
        .description("Utilities for key-value operations.")
        .type_mut(
            "KvRecord",
            "KvRecord represents a key-value record with metadata.",
            |t| {
                t
                .example(std::sync::Arc::new(KvRecord::default()))
                .field("key", |f| f.typ("string").description("The key of the record."))
                .field("value", |f| f.typ("any").description("The value of the record."))
                .field("exists", |f| f.typ("bool").description("Whether the record exists."))
                .field("created_at", |f| f.typ("datetime").description("The time the record was created."))
                .field("last_updated_at", |f| f.typ("datetime").description("The time the record was last updated."))
            },
        )
        .type_mut(
            "KvExecutor",
            "KvExecutor allows templates to get, store and find persistent data within a server.",
            |mut t| {
                t
                .field("guild_id", |f| f.typ("string").description("The guild ID the executor will perform key-value operations on."))
                .field("origin_guild_id", |f| f.typ("string").description("The originating guild ID (the guild ID of the template itself)."))
                .field("scope", |f| f.typ("string").description("The scope of the executor."))
                .method_mut("find", |mut m| {
                    m
                    .parameter("key", |p| p.typ("string")
                    .description("The key to search for. % matches zero or more characters; _ matches a single character. To search anywhere in a string, surround {KEY} with %, e.g. %{KEY}%"))
                    .return_("records", |r| r.typ("{KvRecord}").description("The records found."))
                    .is_promise(true)
                })
                .method_mut("get", |mut m| {
                    m.parameter("key", |p| p.typ("string").description("The key to get."))
                    .return_("value", |r| r.typ("any").description("The value of the key."))
                    .return_("exists", |r| r.typ("bool").description("Whether the key exists."))
                    .is_promise(true)
                })
                .method_mut("getrecord", |mut m| {
                    m.parameter("key", |p| p.typ("string").description("The key to get."))
                    .return_("record", |r| r.typ("KvRecord").description("The record of the key."))
                    .is_promise(true)
                })
                .method_mut("set", |mut m| {
                    m.parameter("key", |p| p.typ("string").description("The key to set."))
                    .parameter("value", |p| p.typ("any").description("The value to set."))
                    .is_promise(true)
                })
                .method_mut("delete", |mut m| {
                    m.parameter("key", |p| p.typ("string").description("The key to delete."))
                    .is_promise(true)
                })
            },
        )
        .method_mut("new", |mut m| {
            m.parameter("token", |p| p.typ("TemplateContext").description("The token of the template to use."))
            .parameter("scope", |p| p.typ("string?").description("The scope of the executor. `this_guild` to use the originating guilds data, `owner_guild` to use the KV of the guild that owns the template on the shop. Defaults to `this_guild` if not specified."))
            .return_("executor", |r| r.typ("KvExecutor").description("A key-value executor."))
        })
}

impl LuaUserData for KvExecutor {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("guild_id", |_, this| Ok(this.guild_id.to_string()));
        fields.add_field_method_get("origin_guild_id", |_, this| Ok(this.origin_guild_id.to_string()));
        fields.add_field_method_get("scope", |_, this| Ok(this.scope.to_string()));
    }

    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("find", |_, this, key: String| {
            Ok(lua_promise!(this, key, |lua, this, key|, {
                this.check("find".to_string(), key.clone())
                .map_err(|e| LuaError::runtime(e.to_string()))?;

                // Check key length
                if key.len() > this.kv_constraints.max_key_length {
                    return Err(LuaError::external("Key length too long"));
                }

                let rec = sqlx::query!(
                    "SELECT key, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key ILIKE $2",
                    this.guild_id.to_string(),
                    key
                )
                .fetch_all(&this.pool)
                .await
                .map_err(|e| LuaError::runtime(e.to_string()))?;

                let mut records = vec![];

                for rec in rec {
                    let record = KvRecord {
                        key: rec.key,
                        value: rec.value.unwrap_or(serde_json::Value::Null),
                        exists: true,
                        created_at: Some(rec.created_at),
                        last_updated_at: Some(rec.last_updated_at),
                    };

                    records.push(record);
                }

                let records: LuaValue = lua.to_value(&records)?;

                Ok(records)
            }))
        });

        methods.add_method("get", |_, this, key: String| {
            Ok(lua_promise!(this, key, |lua, this, key|, {
                this.check("get".to_string(), key.clone())
                    .map_err(|e| LuaError::runtime(e.to_string()))?;

                // Check key length
                if key.len() > this.kv_constraints.max_key_length {
                    return Err(LuaError::external("Key length too long"));
                }

                let rec = sqlx::query!(
                    "SELECT value FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
                    this.guild_id.to_string(),
                    key
                )
                .fetch_optional(&this.pool)
                .await;

                match rec {
                    // Return None and true if record was found but value is null
                    Ok(Some(rec)) => match rec.value {
                        Some(value) => {
                            let value: LuaValue = lua.to_value(&value)?;
                            Ok((Some(value), true))
                        }
                        None => Ok((None, true)),
                    },
                    // Return None and 0 if record was not found
                    Ok(None) => Ok((None, false)),
                    // Return error if query failed
                    Err(e) => Err(LuaError::external(e)),
                }
            }))
        });

        methods.add_method("getrecord", |_, this, key: String| {
            Ok(lua_promise!(this, key, |lua, this, key|, {
                this.check("get".to_string(), key.clone())
                .map_err(|e| LuaError::runtime(e.to_string()))?;
    
                // Check key length
                if key.len() > this.kv_constraints.max_key_length {
                    return Err(LuaError::external("Key length too long"));
                }    

                let rec = sqlx::query!(
                    "SELECT value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
                    this.guild_id.to_string(),
                    key
                )
                .fetch_optional(&this.pool)
                .await;

                let record = match rec {
                    Ok(Some(rec)) => KvRecord {
                        key,
                        value: rec.value.unwrap_or(serde_json::Value::Null),
                        exists: true,
                        created_at: Some(rec.created_at),
                        last_updated_at: Some(rec.last_updated_at),
                    },
                    Ok(None) => KvRecord {
                        key,
                        value: serde_json::Value::Null,
                        exists: false,
                        created_at: None,
                        last_updated_at: None,
                    },
                    Err(e) => return Err(LuaError::external(e)),
                };

                let record: LuaValue = lua.to_value(&record)?;
                Ok(record)
            }))
        });

        methods.add_method("set", |_, this, (key, value): (String, LuaValue)| {
            Ok(lua_promise!(this, key, value, |lua, this, key, value|, {
                this.check("set".to_string(), key.clone())
                .map_err(|e| LuaError::runtime(e.to_string()))?;    
            
                let data = lua.from_value::<serde_json::Value>(value)?;
            
                // Check key length
                if key.len() > this.kv_constraints.max_key_length {
                    return Err(LuaError::external("Key length too long"));
                }
    
                // Check bytes length
                let data_str = serde_json::to_string(&data)
                    .map_err(|e| LuaError::runtime(e.to_string()))?;
    
                if data_str.len() > this.kv_constraints.max_value_bytes {
                    return Err(LuaError::external("Value length too long"));
                }
    
                let mut tx = this.pool.begin().await
                    .map_err(|e| LuaError::runtime(e.to_string()))?;
    
                let rec = sqlx::query!(
                    "SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1",
                    this.guild_id.to_string(),
                )
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| LuaError::runtime(e.to_string()))?;
    
                if rec.count.unwrap_or(0) >= this.kv_constraints.max_keys.try_into().map_err(|e: TryFromIntError| LuaError::runtime(e.to_string()))? {
                    return Err(LuaError::external("Max keys limit reached"));
                }
    
                sqlx::query!(
                    "INSERT INTO guild_templates_kv (guild_id, key, value) VALUES ($1, $2, $3) ON CONFLICT (guild_id, key) DO UPDATE SET value = $3, last_updated_at = NOW()",
                    this.guild_id.to_string(),
                    key,
                    data,
                )
                .execute(&mut *tx)
                .await
                .map_err(|e| LuaError::runtime(e.to_string()))?;
    
                tx.commit().await
                .map_err(|e| LuaError::runtime(e.to_string()))?;
    
                Ok(())    
            }))
        });

        methods.add_method("delete", |_lua, this, key: String| {
            Ok(lua_promise!(this, key, |_lua, this, key|, {
                this.check("delete".to_string(), key.clone())
                .map_err(|e| LuaError::runtime(e.to_string()))?;
                
                // Check key length
                if key.len() > this.kv_constraints.max_key_length {
                    return Err(LuaError::external("Key length too long"));
                }
    
                sqlx::query!(
                    "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
                    this.guild_id.to_string(),
                    key,
                )
                .execute(&this.pool)
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
        lua.create_function(|_, (token, scope): (TemplateContextRef, Option<String>)| {
            let scope = ExecutorScope::scope_str(scope)?;
            let guild_id = scope.guild(&token);
            let executor = KvExecutor {
                allowed_caps: token.template_data.allowed_caps.clone(),
                origin_guild_id: token.guild_state.guild_id,
                guild_id,
                scope,
                pool: token.guild_state.pool.clone(),
                ratelimits: token.guild_state.ratelimits.clone(),
                kv_constraints: token.guild_state.kv_constraints,
            };

            Ok(executor)
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
