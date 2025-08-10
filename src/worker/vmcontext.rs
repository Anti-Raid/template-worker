use super::workerstate::WorkerState;
use super::workervmmanager::Id;
use super::template::Template;
use crate::worker::keyexpirychannel::KeyExpiryChannel;
use crate::worker::workercachedata::WorkerCacheData;
use khronos_runtime::traits::context::{
    CompatibilityFlags, KhronosContext, Limitations, ScriptData,
};
use khronos_runtime::traits::datastoreprovider::{DataStoreImpl, DataStoreProvider};
use khronos_runtime::traits::discordprovider::DiscordProvider;
use khronos_runtime::traits::httpclientprovider::HTTPClientProvider;
use khronos_runtime::traits::httpserverprovider::HTTPServerProvider;
use khronos_runtime::traits::ir::kv::KvRecord;
use khronos_runtime::traits::ir::ObjectMetadata;
use khronos_runtime::traits::kvprovider::KVProvider;
use khronos_runtime::traits::objectstorageprovider::ObjectStorageProvider;
use khronos_runtime::utils::khronos_value::KhronosValue;
use rand::distr::{Alphanumeric, SampleString};
use serde_json::Value;
use sqlx::Row;
use std::{rc::Rc, sync::Arc};
use super::limits::{LuaKVConstraints, Ratelimits};

/// Returns a random string of length ``length``
fn gen_random(length: usize) -> String {
    Alphanumeric.sample_string(&mut rand::rng(), length)
}

#[derive(Clone)]
pub struct TemplateContextProvider {
    state: WorkerState,

    id: Id,

    /// The template data
    template_data: Arc<Template>,

    /// The datastores to expose
    datastores: Vec<Rc<dyn DataStoreImpl>>,

    /// Script data
    script_data: Arc<ScriptData>,

    /// The KV constraints for this template
    kv_constraints: LuaKVConstraints,
    
    /// The ratelimits of the VM
    ratelimits: Rc<Ratelimits>,

    /// The key expiry channel of the worker
    key_expiry_chan: KeyExpiryChannel,
}

impl TemplateContextProvider {
    fn datastores(
        state: WorkerState,
        cache: WorkerCacheData,
        id: Id,
        _template_data: Arc<Template>,
    ) -> Vec<Rc<dyn DataStoreImpl>> {
        vec![
            Rc::new(super::vmdatastores::StatsStore {
                state: state.clone(),
            }),
            Rc::new(super::vmdatastores::LinksStore {}),
            Rc::new(khronos_runtime::traits::ir::datastores::CopyDataStore {}),
            Rc::new(super::vmdatastores::TemplateStore {
                state,
                cache,
                id
            }),
        ]
    }

    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(
        state: WorkerState, 
        template_data: Arc<Template>, 
        cache: WorkerCacheData, 
        id: Id,
        kv_constraints: LuaKVConstraints,
        ratelimits: Rc<Ratelimits>,
        key_expiry_chan: KeyExpiryChannel,
    ) -> Self {
        Self {
            id,
            datastores: Self::datastores(state.clone(), cache, id, template_data.clone()),
            state,
            script_data: Arc::new(ScriptData {
                guild_id: Some(template_data.guild_id),
                name: template_data.name.clone(),
                description: template_data.description.clone(),
                shop_name: template_data.shop_name.clone(),
                shop_owner: template_data.shop_owner,
                events: template_data.events.clone(),
                error_channel: template_data.error_channel.map(|x| x.widen()),
                lang: template_data.lang.to_string(),
                allowed_caps: template_data.allowed_caps.clone(),
                created_by: None,
                created_at: Some(template_data.created_at),
                updated_by: None,
                updated_at: Some(template_data.updated_at),
                compatibility_flags: CompatibilityFlags::empty(),
            }),
            template_data,
            kv_constraints,
            ratelimits,
            key_expiry_chan,
        }
    }
}

impl KhronosContext for TemplateContextProvider {
    type KVProvider = ArKVProvider;
    type DiscordProvider = ArDiscordProvider;
    type DataStoreProvider = ArDataStoreProvider;
    type ObjectStorageProvider = ArObjectStorageProvider;
    type HTTPClientProvider = ArHTTPClientProvider;
    type HTTPServerProvider = ArHTTPServerProvider;

    fn data(&self) -> &ScriptData {
        &self.script_data
    }

    fn limitations(&self) -> Limitations {
        Limitations::new(self.template_data.allowed_caps.clone())
    }

    fn guild_id(&self) -> Option<serenity::all::GuildId> {
        match self.id {
            Id::GuildId(gid) => Some(gid),
        }
    }

    fn owner_guild_id(&self) -> Option<serenity::all::GuildId> {
        self.template_data.shop_owner
    }

    fn template_name(&self) -> String {
        self.template_data.name.clone()
    }

    fn current_user(&self) -> Option<serenity::all::CurrentUser> {
        Some(
            (*self.state
            .current_user)
            .clone()
        )
    }

    fn kv_provider(&self) -> Option<Self::KVProvider> {
        Some(ArKVProvider {
            guild_id: self.guild_id()?,
            state: self.state.clone(),
            kv_constraints: self.kv_constraints.clone(),
            ratelimits: self.ratelimits.clone(),
            key_expiry_chan: self.key_expiry_chan.clone(),
        })
    }

    fn discord_provider(&self) -> Option<Self::DiscordProvider> {
        Some(ArDiscordProvider {
            guild_id: self.guild_id()?,
            state: self.state.clone(),
            ratelimits: self.ratelimits.clone(),
        })
    }

    fn datastore_provider(&self) -> Option<Self::DataStoreProvider> {
        Some(ArDataStoreProvider {
            guild_id: self.guild_id()?,
            state: self.state.clone(),
            datastores: self.datastores.clone(),
            ratelimits: self.ratelimits.clone(),
        })
    }

    fn objectstorage_provider(&self) -> Option<Self::ObjectStorageProvider> {
        Some(ArObjectStorageProvider {
            guild_id: self.guild_id()?,
            state: self.state.clone(),
            kv_constraints: self.kv_constraints.clone(),
            ratelimits: self.ratelimits.clone(),
        })
    }

    fn httpclient_provider(&self) -> Option<Self::HTTPClientProvider> {
        Some(ArHTTPClientProvider {
            guild_id: self.guild_id()?,
            state: self.state.clone(),
            ratelimits: self.ratelimits.clone(),
        })
    }

    fn httpserver_provider(&self) -> Option<Self::HTTPServerProvider> {
        None // Don't expose HTTP sercer provider in templates
    }
}

#[derive(Clone)]
pub struct ArKVProvider {
    guild_id: serenity::all::GuildId,
    state: WorkerState,
    kv_constraints: LuaKVConstraints,
    ratelimits: Rc<Ratelimits>,
    key_expiry_chan: KeyExpiryChannel,
}

impl KVProvider for ArKVProvider {
    fn attempt_action(&self, _scope: &[String], bucket: &str) -> Result<(), crate::Error> {
        self.ratelimits.kv.check(bucket)
    }

    async fn get(&self, scopes: &[String], key: String) -> Result<Option<KvRecord>, crate::Error> {
        // Check key length
        if key.len() > self.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let rec = if scopes.is_empty() {
            sqlx::query(
            "SELECT id, expires_at, scopes, value, created_at, last_updated_at, resume FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
            )
            .bind(self.guild_id.to_string())
            .bind(&key)
            .fetch_optional(&self.state.pool)
            .await?
        } else {
            sqlx::query(
            "SELECT id, expires_at, scopes, value, created_at, last_updated_at, resume FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3",
            )
            .bind(self.guild_id.to_string())
            .bind(&key)
            .bind(scopes)
            .fetch_optional(&self.state.pool)
            .await?
        };

        let Some(rec) = rec else {
            return Ok(None);
        };

        Ok(Some(KvRecord {
            id: rec.try_get::<String, _>("id")?,
            key,
            scopes: rec.try_get::<Vec<String>, _>("scopes")?,
            value: {
                let value = rec
                    .try_get::<Option<serde_json::Value>, _>("value")?
                    .unwrap_or(serde_json::Value::Null);

                serde_json::from_value(value)
                    .map_err(|e| format!("Failed to deserialize value: {}", e))?
            },
            created_at: Some(rec.try_get("created_at")?),
            last_updated_at: Some(rec.try_get("last_updated_at")?),
            expires_at: rec.try_get("expires_at")?,
            resume: rec.try_get("resume").unwrap_or(false),
        }))
    }

    async fn get_by_id(&self, id: String) -> Result<Option<KvRecord>, crate::Error> {
        let rec = sqlx::query(
            "SELECT key, expires_at, scopes, value, created_at, last_updated_at, resume FROM guild_templates_kv WHERE guild_id = $1 AND id = $2",
        )
        .bind(self.guild_id.to_string())
        .bind(id.clone())
        .fetch_optional(&self.state.pool)
        .await?;

        let Some(rec) = rec else {
            return Ok(None);
        };

        Ok(Some(KvRecord {
            id,
            key: rec.try_get("key")?,
            scopes: rec.try_get::<Vec<String>, _>("scopes")?,
            value: {
                let value = rec
                    .try_get::<Option<serde_json::Value>, _>("value")?
                    .unwrap_or(serde_json::Value::Null);

                serde_json::from_value(value)
                    .map_err(|e| format!("Failed to deserialize value: {}", e))?
            },
            created_at: Some(rec.try_get("created_at")?),
            last_updated_at: Some(rec.try_get("last_updated_at")?),
            expires_at: rec.try_get("expires_at")?,
            resume: rec.try_get("resume").unwrap_or(false),
        }))
    }

    async fn list_scopes(&self) -> Result<Vec<String>, crate::Error> {
        let rec = sqlx::query(
            "SELECT DISTINCT unnest_scope AS scope
FROM guild_templates_kv, unnest(scopes) AS unnest_scope
ORDER BY scope",
        )
        .bind(self.guild_id.to_string())
        .fetch_all(&self.state.pool)
        .await?;

        let mut scopes = vec![];

        for rec in rec {
            scopes.push(rec.try_get("scope")?);
        }

        Ok(scopes)
    }

    async fn set(
        &self,
        scopes: &[String],
        key: String,
        data: KhronosValue,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        resume: bool,
    ) -> Result<(bool, String), crate::Error> {
        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        // Check key length
        if key.len() > self.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        // Check bytes length
        let data_str = serde_json::to_string(&data)?;

        if data_str.len() > self.kv_constraints.max_value_bytes {
            return Err("Value length too long".into());
        }

        let mut tx = self.state.pool.begin().await?;

        let (curr_id, curr_expiry) = {
            let row = sqlx::query(
                "SELECT id, expires_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3",
            )
            .bind(self.guild_id.to_string())
            .bind(&key)
            .bind(scopes)
            .fetch_optional(&mut *tx)
            .await?;

            match row {
                Some(row) => (
                    Some(row.try_get::<String, _>("id")?),
                    row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("expires_at")?,
                ),
                None => (None, None),
            }
        };

        let (exists, id) = if let Some(curr_id) = curr_id {
            // Update existing record
            sqlx::query(
                "UPDATE guild_templates_kv SET value = $1, last_updated_at = NOW(), expires_at = $2, resume = $3 WHERE id = $4",
            )
            .bind(serde_json::to_value(data)?)
            .bind(expires_at)
            .bind(resume)
            .bind(&curr_id)
            .execute(&mut *tx)
            .await?;

            (true, curr_id)
        } else {
            // Insert new record
            let id = gen_random(64);
            sqlx::query(
                "INSERT INTO guild_templates_kv (id, guild_id, key, value, scopes, expires_at, resume) VALUES ($1, $2, $3, $4, $5, $6, $7)",
            )
            .bind(&id)
            .bind(self.guild_id.to_string())
            .bind(key)
            .bind(serde_json::to_value(data)?)
            .bind(scopes)
            .bind(expires_at)
            .bind(resume)
            .execute(&mut *tx)
            .await?;

            (false, id)
        };

        tx.commit().await?;

        if curr_expiry != expires_at {
            // Regenerate the cache if the expiry has changed
            self.key_expiry_chan.repopulate()?;
        }

        Ok((exists, id))
    }

    async fn set_expiry(
        &self,
        scopes: &[String],
        key: String,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), khronos_runtime::Error> {
        // Check key length
        if key.len() > self.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        if scopes.is_empty() {
            sqlx::query(
            "UPDATE guild_templates_kv SET expires_at = $1, last_updated_at = NOW() WHERE guild_id = $2 AND key = $3",
            )
            .bind(expires_at)
            .bind(self.guild_id.to_string())
            .bind(&key)
            .execute(&self.state.pool)
            .await?;
        } else {
            sqlx::query(
                "UPDATE guild_templates_kv SET expires_at = $1, last_updated_at = NOW() WHERE guild_id = $2 AND key = $3 AND scopes @> $4",
            )
            .bind(expires_at)
            .bind(self.guild_id.to_string())
            .bind(&key)
            .bind(scopes)
            .execute(&self.state.pool)
            .await?;
        }

        // Regenerate the cache in any case
        self.key_expiry_chan.repopulate()?;

        Ok(())
    }

    async fn set_expiry_by_id(
        &self,
        id: String,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), khronos_runtime::Error> {
        sqlx::query(
            "UPDATE guild_templates_kv SET expires_at = $1, last_updated_at = NOW() WHERE guild_id = $2 AND id = $3",
        )
        .bind(expires_at)
        .bind(self.guild_id.to_string())
        .bind(id)
        .execute(&self.state.pool)
        .await?;

        // Regenerate the cache in any case
        self.key_expiry_chan.repopulate()?;

        Ok(())
    }

    async fn set_by_id(
        &self,
        id: String,
        data: KhronosValue,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        resume: bool,
    ) -> Result<(), khronos_runtime::Error> {
        // Check bytes length
        let data_str = serde_json::to_string(&data)?;

        if data_str.len() > self.kv_constraints.max_value_bytes {
            return Err("Value length too long".into());
        }

        let mut tx = self.state.pool.begin().await?;

        let curr_expiry = {
            let row = sqlx::query(
                "SELECT expires_at FROM guild_templates_kv WHERE guild_id = $1 AND id = $2",
            )
            .bind(self.guild_id.to_string())
            .bind(&id)
            .fetch_optional(&mut *tx)
            .await?;

            let Some(row) = row else {
                return Ok(()); // do nothing if the record doesn't exist. TODO: rethink this behavior
            };

            row.try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("expires_at")?
        };

        // Update existing record
        sqlx::query(
            "UPDATE guild_templates_kv SET value = $1, last_updated_at = NOW(), expires_at = $2, resume = $3 WHERE id = $4",
        )
        .bind(serde_json::to_value(data)?)
        .bind(expires_at)
        .bind(resume)
        .bind(&id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        if curr_expiry != expires_at {
            // Regenerate the cache if the expiry has changed
            self.key_expiry_chan.repopulate()?;
        }

        Ok(())
    }

    async fn delete(&self, scopes: &[String], key: String) -> Result<(), crate::Error> {
        // Check key length
        if key.len() > self.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let rows = if scopes.is_empty() {
            sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 RETURNING expires_at",
            )
            .bind(self.guild_id.to_string())
            .bind(key)
            .fetch_all(&self.state.pool)
            .await?
        } else {
            sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3 RETURNING expires_at",
            )
            .bind(self.guild_id.to_string())
            .bind(key)
            .bind(scopes)
            .fetch_all(&self.state.pool)
            .await?
        };

        for row in rows {
            let expires_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("expires_at")?;

            if expires_at.is_some() {
                // Regenerate the cache if the key has an expiry set
                self.key_expiry_chan.repopulate()?;
                break; // No need to continue if we found at least one expiry
            }
        }

        Ok(())
    }

    async fn delete_by_id(&self, id: String) -> Result<(), crate::Error> {
        let row = sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND id = $2 RETURNING expires_at",
        )
        .bind(self.guild_id.to_string())
        .bind(id)
        .fetch_optional(&self.state.pool)
        .await?;

        if let Some(row) = row {
            let expires_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("expires_at")?;

            if expires_at.is_some() {
                // Regenerate the cache if the key has an expiry set
                self.key_expiry_chan.repopulate()?;
            }
        }

        Ok(())
    }

    async fn find(&self, scopes: &[String], query: String) -> Result<Vec<KvRecord>, crate::Error> {
        // Check key length
        if query.len() > self.kv_constraints.max_key_length {
            return Err("Query length too long".into());
        }

        let rec = {
            if query == "%%" {
                // Fast path, omit ILIKE if '%%' is used
                if scopes.is_empty() {
                    // no query, no scopes
                    sqlx::query(
                    "SELECT id, key, value, expires_at, created_at, last_updated_at, scopes, resume FROM guild_templates_kv WHERE guild_id = $1",
                    )
                    .bind(self.guild_id.to_string())
                    .fetch_all(&self.state.pool)
                    .await?
                } else {
                    // no query, scopes
                    sqlx::query(
                    "SELECT id, key, value, expires_at, created_at, last_updated_at, scopes, resume FROM guild_templates_kv WHERE guild_id = $1 AND scopes @> $2",
                    )
                    .bind(self.guild_id.to_string())
                    .bind(scopes)
                    .fetch_all(&self.state.pool)
                    .await?
                }
            } else {
                if scopes.is_empty() {
                    // query, no scopes
                    sqlx::query(
                    "SELECT id, key, value, expires_at, created_at, last_updated_at, scopes, resume FROM guild_templates_kv WHERE guild_id = $1 AND key ILIKE $2",
                    )
                    .bind(self.guild_id.to_string())
                    .bind(query)
                    .fetch_all(&self.state.pool)
                    .await?
                } else {
                    // query, scopes
                    sqlx::query(
                    "SELECT id, key, value, expires_at, created_at, last_updated_at, scopes, resume FROM guild_templates_kv WHERE guild_id = $1 AND scopes @> $2 AND key ILIKE $3",
                    )
                    .bind(self.guild_id.to_string())
                    .bind(scopes)
                    .bind(query)
                    .fetch_all(&self.state.pool)
                    .await?
                }
            }
        };

        let mut records = vec![];

        for rec in rec {
            let record = KvRecord {
                id: rec.try_get::<String, _>("id")?,
                scopes: rec.try_get::<Vec<String>, _>("scopes")?,
                expires_at: rec.try_get("expires_at")?,
                key: rec.try_get("key")?,
                value: {
                    let rec = rec
                        .try_get::<Option<serde_json::Value>, _>("value")?
                        .unwrap_or(serde_json::Value::Null);

                    serde_json::from_value(rec)
                        .map_err(|e| format!("Failed to deserialize value: {}", e))?
                },
                created_at: Some(rec.try_get("created_at")?),
                last_updated_at: Some(rec.try_get("last_updated_at")?),
                resume: rec.try_get("resume").unwrap_or(false),
            };

            records.push(record);
        }

        Ok(records)
    }

    async fn exists(&self, scopes: &[String], key: String) -> Result<bool, crate::Error> {
        // Check key length
        if key.len() > self.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let rec = sqlx::query(
            "SELECT id FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3",
        )
        .bind(self.guild_id.to_string())
        .bind(key)
        .bind(scopes)
        .fetch_optional(&self.state.pool)
        .await?
        .is_some();

        Ok(rec)
    }

    async fn keys(&self, scopes: &[String]) -> Result<Vec<String>, crate::Error> {
        let rec =
            sqlx::query("SELECT key FROM guild_templates_kv WHERE guild_id = $1 AND scopes @> $2")
                .bind(self.guild_id.to_string())
                .bind(scopes)
                .fetch_all(&self.state.pool)
                .await?;

        let mut keys = vec![];

        for rec in rec {
            keys.push(rec.try_get("key")?);
        }

        Ok(keys)
    }
}

#[derive(Clone)]
pub struct ArDiscordProvider {
    guild_id: serenity::all::GuildId,
    state: WorkerState,
    ratelimits: Rc<Ratelimits>,
}

impl DiscordProvider for ArDiscordProvider {
    fn attempt_action(&self, bucket: &str) -> serenity::Result<(), crate::Error> {
        self.ratelimits.discord.check(bucket)
    }

    async fn get_guild(
        &self,
    ) -> serenity::Result<Value, crate::Error> {
        Ok(crate::sandwich::guild(
            &self.state.serenity_http,
            &self.state.reqwest_client,
            self.guild_id,
        )
        .await
        .map_err(|e| format!("Failed to fetch guild information from sandwich: {}", e))?)
    }

    async fn get_guild_member(
        &self,
        user_id: serenity::all::UserId,
    ) -> serenity::Result<Value, crate::Error> {
        let member = crate::sandwich::member_in_guild(
            &self.state.serenity_http,
            &self.state.reqwest_client,
            self.guild_id,
            user_id,
        )
        .await
        .map_err(|e| format!("Failed to fetch member information from sandwich: {}", e))?;

        let Some(member) = member else {
            return Ok(Value::Null);
        };

        return Ok(member)
    }

    async fn get_guild_channels(
        &self,
    ) -> serenity::Result<Value, crate::Error> {
        let channels = crate::sandwich::guild_channels(
            &self.state.serenity_http,
            &self.state.reqwest_client,
            self.guild_id,
        )
        .await
        .map_err(|e| format!("Failed to fetch channel information from sandwich: {}", e))?;

        Ok(channels)
    }

    async fn get_guild_roles(
        &self,
    ) -> serenity::Result<Value, crate::Error>
    {
        let roles = crate::sandwich::guild_roles(
            &self.state.serenity_http,
            &self.state.reqwest_client,
            self.guild_id,
        )
        .await
        .map_err(|e| format!("Failed to fetch role information from sandwich: {}", e))?;

        Ok(roles)
    }

    async fn get_channel(
        &self,
        channel_id: serenity::all::GenericChannelId,
    ) -> serenity::Result<Value, crate::Error> {
        let channel = crate::sandwich::channel(
            &self.state.serenity_http,
            &self.state.reqwest_client,
            Some(self.guild_id),
            channel_id,
        )
        .await
        .map_err(|e| format!("Failed to fetch channel information from sandwich: {}", e))?;

        let Some(channel) = channel else {
            return Err("Channel not found".into());
        };

        let Some(Value::String(guild_id)) = channel.get("guild_id") else {
            return Err(format!("Channel {channel_id} does not belong to a guild").into());
        };

        if guild_id != &self.guild_id.to_string() {
            return Err(format!("Channel {channel_id} does not belong to the guild").into());
        }

        Ok(channel)
    }

    fn guild_id(&self) -> serenity::all::GuildId {
        self.guild_id
    }

    fn serenity_http(&self) -> &serenity::http::Http {
        &self.state.serenity_http
    }

    async fn edit_channel_permissions(
        &self,
        channel_id: serenity::all::GenericChannelId,
        target_id: serenity::all::TargetId,
        data: impl serde::Serialize,
        audit_log_reason: Option<&str>,
    ) -> Result<(), khronos_runtime::Error> {
        self.state
            .serenity_http
            .create_permission(channel_id.expect_channel(), target_id, &data, audit_log_reason)
            .await
            .map_err(|e| format!("Failed to edit channel permissions: {}", e))?;

        Ok(())
    }

    async fn edit_channel(
        &self,
        channel_id: serenity::all::GenericChannelId,
        map: impl serde::Serialize,
        audit_log_reason: Option<&str>,
    ) -> Result<Value, crate::Error> {
        let chan = self
            .state
            .serenity_http
            .edit_channel(channel_id, &map, audit_log_reason)
            .await
            .map_err(|e| format!("Failed to edit channel: {}", e))?;

        Ok(chan)
    }

    async fn delete_channel(
        &self,
        channel_id: serenity::all::GenericChannelId,
        audit_log_reason: Option<&str>,
    ) -> Result<Value, crate::Error> {
        let chan = self
            .state
            .serenity_http
            .delete_channel(channel_id, audit_log_reason)
            .await
            .map_err(|e| format!("Failed to delete channel: {}", e))?;

        Ok(chan)
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ArDataStoreProvider {
    guild_id: serenity::all::GuildId,
    state: WorkerState,
    datastores: Vec<Rc<dyn DataStoreImpl>>,
    ratelimits: Rc<Ratelimits>,
}

impl DataStoreProvider for ArDataStoreProvider {
    fn attempt_action(&self, method: &str, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.ratelimits
            .data_stores
            .check(&format!("{}:{}", method, bucket))
    }

    /// Returns a builtin data store given its name
    fn get_builtin_data_store(&self, name: &str) -> Option<Rc<dyn DataStoreImpl>> {
        for ds in self.datastores.iter() {
            if ds.name() == name {
                return Some(ds.clone());
            }
        }

        None
    }

    /// Returns all public builtin data stores
    fn public_builtin_data_stores(&self) -> Vec<String> {
        self.datastores.iter().map(|ds| ds.name()).collect()
    }
}

#[derive(Clone)]
pub struct ArObjectStorageProvider {
    guild_id: serenity::all::GuildId,
    ratelimits: Rc<Ratelimits>,
    state: WorkerState,
    kv_constraints: LuaKVConstraints,
}

impl ObjectStorageProvider for ArObjectStorageProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.ratelimits.object_storage.check(bucket)
    }

    fn bucket_name(&self) -> String {
        crate::objectstore::guild_bucket(self.guild_id)
    }

    async fn list_files(
        &self,
        prefix: Option<String>,
    ) -> Result<Vec<ObjectMetadata>, khronos_runtime::Error> {
        Ok(self
            .state
            .object_store
            .list_files(
                &crate::objectstore::guild_bucket(self.guild_id),
                prefix.as_ref().map(|x| x.as_str()),
            )
            .await?
            .into_iter()
            .map(|x| ObjectMetadata {
                key: x.key,
                last_modified: x.last_modified,
                size: x.size,
                etag: x.etag,
            })
            .collect::<Vec<_>>())
    }

    async fn file_exists(&self, key: String) -> Result<bool, khronos_runtime::Error> {
        Ok(self
            .state
            .object_store
            .exists(&crate::objectstore::guild_bucket(self.guild_id), &key)
            .await?)
    }

    async fn download_file(&self, key: String) -> Result<Vec<u8>, khronos_runtime::Error> {
        Ok(self
            .state
            .object_store
            .download_file(&crate::objectstore::guild_bucket(self.guild_id), &key)
            .await?)
    }

    async fn get_file_url(
        &self,
        key: String,
        expiry: std::time::Duration,
    ) -> Result<String, khronos_runtime::Error> {
        Ok(self
            .state
            .object_store
            .get_url(
                &crate::objectstore::guild_bucket(self.guild_id),
                &key,
                expiry,
            )
            .await?)
    }

    async fn upload_file(&self, key: String, data: Vec<u8>) -> Result<(), khronos_runtime::Error> {
        if key.len()
            > self
                .kv_constraints
                .max_object_storage_path_length
        {
            return Err("Path length too long".into());
        }

        if data.len() > self.kv_constraints.max_object_storage_bytes {
            return Err("Data too large".into());
        }

        self.state
            .object_store
            .upload_file(&crate::objectstore::guild_bucket(self.guild_id), &key, data)
            .await?;

        Ok(())
    }

    async fn delete_file(&self, key: String) -> Result<(), khronos_runtime::Error> {
        Ok(self
            .state
            .object_store
            .delete(&crate::objectstore::guild_bucket(self.guild_id), &key)
            .await?)
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ArHTTPClientProvider {
    guild_id: serenity::all::GuildId,
    state: WorkerState,
    ratelimits: Rc<Ratelimits>,
}

impl HTTPClientProvider for ArHTTPClientProvider {
    fn attempt_action(&self, bucket: &str, _url: &str) -> Result<(), khronos_runtime::Error> {
        self.ratelimits.http.check(bucket)
    }
}

#[derive(Clone)]
pub struct ArHTTPServerProvider {
}

impl HTTPServerProvider for ArHTTPServerProvider {
    fn attempt_action(&self, _bucket: &str, _path: String) -> Result<(), khronos_runtime::Error> {
        Err("Internal Error: unreachable code: HTTP server provider not implemented for templates".into())
    }
}
