use super::workerstate::WorkerState;
use super::workervmmanager::Id;
use crate::events::AntiraidEvent;
use crate::worker::workerstate::TenantState;
use crate::worker::workervmmanager::VmData;
use super::builtins::{Builtins, BuiltinsPatches, TemplatingTypes};
use crate::worker::keyexpirychannel::KeyExpiryChannel;
use khronos_runtime::core::typesext::Vfs;
use khronos_runtime::mluau_require::create_memory_vfs_from_map;
use khronos_runtime::traits::context::{
    KhronosContext, Limitations,
};
use khronos_runtime::traits::ir::runtime as runtime_ir;
use dapi::controller::DiscordProvider;
use khronos_runtime::traits::httpclientprovider::HTTPClientProvider;
use khronos_runtime::traits::httpserverprovider::HTTPServerProvider;
use khronos_runtime::traits::ir::kv::KvRecord;
use khronos_runtime::traits::ir::ObjectMetadata;
use khronos_runtime::traits::kvprovider::KVProvider;
use khronos_runtime::traits::objectstorageprovider::ObjectStorageProvider;
use khronos_runtime::traits::runtimeprovider::RuntimeProvider;
use khronos_runtime::utils::khronos_value::KhronosValue;
use rand::distr::{Alphanumeric, SampleString};
use serde_json::Value;
use sqlx::Row;
use std::collections::HashSet;
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

    /// The KV constraints for this template
    kv_constraints: LuaKVConstraints,
    
    /// The ratelimits of the VM
    ratelimits: Rc<Ratelimits>,

    /// The key expiry channel of the worker
    key_expiry_chan: KeyExpiryChannel,
}

impl TemplateContextProvider {
    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(
        id: Id,
        vm_data: VmData,
        key_expiry_chan: KeyExpiryChannel,
    ) -> Self {
        Self {
            id,
            state: vm_data.state,
            kv_constraints: vm_data.kv_constraints,
            ratelimits: vm_data.ratelimits,
            key_expiry_chan,
        }
    }
}

impl KhronosContext for TemplateContextProvider {
    type KVProvider = ArKVProvider;
    type DiscordProvider = ArDiscordProvider;
    type ObjectStorageProvider = ArObjectStorageProvider;
    type HTTPClientProvider = ArHTTPClientProvider;
    type HTTPServerProvider = ArHTTPServerProvider;
    type RuntimeProvider = ArRuntimeProvider;

    fn limitations(&self) -> Limitations {
        // We start with full limitations with builtins applying extra limits prior to event dispatch where desired
        Limitations::new(vec!["*".to_string()])
    }

    fn guild_id(&self) -> Option<serenity::all::GuildId> {
        match self.id {
            Id::GuildId(gid) => Some(gid),
        }
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

    fn runtime_provider(&self) -> Option<Self::RuntimeProvider> {
        Some(ArRuntimeProvider {
            id: Id::GuildId(self.guild_id()?),
            state: self.state.clone(),
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
            "SELECT id, expires_at, scopes, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
            )
            .bind(self.guild_id.to_string())
            .bind(&key)
            .fetch_optional(&self.state.pool)
            .await?
        } else {
            sqlx::query(
            "SELECT id, expires_at, scopes, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3",
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
        }))
    }

    async fn get_by_id(&self, id: String) -> Result<Option<KvRecord>, crate::Error> {
        let rec = sqlx::query(
            "SELECT key, expires_at, scopes, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND id = $2",
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
                "UPDATE guild_templates_kv SET value = $1, last_updated_at = NOW(), expires_at = $2 WHERE id = $3",
            )
            .bind(serde_json::to_value(data)?)
            .bind(expires_at)
            .bind(&curr_id)
            .execute(&mut *tx)
            .await?;

            (true, curr_id)
        } else {
            // Insert new record
            let id = gen_random(64);
            sqlx::query(
                "INSERT INTO guild_templates_kv (id, guild_id, key, value, scopes, expires_at) VALUES ($1, $2, $3, $4, $5, $6)",
            )
            .bind(&id)
            .bind(self.guild_id.to_string())
            .bind(key)
            .bind(serde_json::to_value(data)?)
            .bind(scopes)
            .bind(expires_at)
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
            "UPDATE guild_templates_kv SET value = $1, last_updated_at = NOW(), expires_at = $2 WHERE id = $3",
        )
        .bind(serde_json::to_value(data)?)
        .bind(expires_at)
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

    fn current_user(&self) -> Option<serenity::all::CurrentUser> {
        Some(
            (*self.state
            .current_user)
            .clone()
        )
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

#[derive(Clone)]
pub struct ArRuntimeProvider {
    id: Id,
    state: WorkerState,
    ratelimits: Rc<Ratelimits>,
}

impl ArRuntimeProvider {
    /// Returns the built-in template's runtime ir
    fn builtins_template(&self) -> runtime_ir::Template {
        runtime_ir::Template {
            id: "$builtins".to_string(),
            owner: match self.id {
                Id::GuildId(gid) => runtime_ir::TemplateOwner::Guild { id: gid }
            },
            created_at: chrono::Utc::now(),
            last_updated_at: chrono::Utc::now(),
            source: runtime_ir::TemplateSource::Builtins,
            allowed_caps: vec!["*".to_string()],
            vfs: Vfs {
                vfs: Arc::new(vfs::OverlayFS::new(&vec![
                    vfs::EmbeddedFS::<BuiltinsPatches>::new().into(),
                    vfs::EmbeddedFS::<Builtins>::new().into(),
                    vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
                ]))
            },
            paused: false,
        }
    }

    /// Converts a database row into a runtime ir template
    fn template_from_row(
        &self,
        row: &sqlx::postgres::PgRow,
    ) -> Result<runtime_ir::Template, khronos_runtime::Error> {
        let owner = match self.id {
            Id::GuildId(gid) => runtime_ir::TemplateOwner::Guild { id: gid },
        };

        let source: String = row.try_get("source")?;
        let (template_source, vfs) = match source.as_str() {
            "custom" => {
                let content_json: serde_json::Value = row.try_get("content")?;
                let content = serde_json::from_value(content_json).map_err(|e| {
                    format!("Failed to deserialize template content: {}", e)
                })?;

                // VFS for a custom template
                let vfs = Arc::new(vfs::OverlayFS::new(&vec![
                    create_memory_vfs_from_map(&content)?.into(),
                    vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
                ]));

                (runtime_ir::TemplateSource::Custom {
                    name: row.try_get("name")?,
                    language: row.try_get("language")?,
                    content,
                }, vfs)
            }
            _ => {
                return Err(format!("Unknown template source: {}", source).into());
            }
        };
        Ok(runtime_ir::Template {
            id: row.try_get("id")?,
            owner,
            created_at: row.try_get("created_at")?,
            last_updated_at: row.try_get("last_updated_at")?,
            source: template_source,
            allowed_caps: row.try_get("allowed_caps")?,
            vfs: Vfs {
                vfs,
            },
            paused: false,
        })
    }
}

impl RuntimeProvider for ArRuntimeProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.ratelimits.runtime.check(bucket)
    }

    async fn list_templates(&self) -> Result<Vec<khronos_runtime::traits::ir::runtime::Template>, khronos_runtime::Error> {
        let mut base_templates = vec![self.builtins_template()];

        let rows = sqlx::query(
            r#"
                SELECT id, source,
                    -- data custom
                    name, language, content,
                    -- data shop
                    shop_ref,
                    -- metadata
                    created_at, last_updated_at, allowed_caps, events, state
                FROM attached_templates 
                WHERE owner_type = $1
                AND owner_id = $2"#,
        )
        .bind(self.id.tenant_type())
        .bind(self.id.tenant_id())
        .fetch_all(&self.state.pool)
        .await?;

        for row in rows.iter() {
            base_templates.push(self.template_from_row(row)?);
        }

        Ok(base_templates)
    }

    async fn get_template(&self, id: &str) -> Result<Option<khronos_runtime::traits::ir::runtime::Template>, khronos_runtime::Error> {
        let id: sqlx::types::Uuid = id.parse().map_err(|_| "Invalid template ID")?;
        let Some(row) = sqlx::query(
            r#"
                SELECT id, source,
                    -- data custom
                    name, language, content,
                    -- data shop
                    shop_ref,
                    -- metadata
                    created_at, last_updated_at, allowed_caps, events, state
                FROM attached_templates 
                WHERE owner_type = $1
                AND owner_id = $2
                AND id = $3"#,
        )
        .bind(self.id.tenant_type())
        .bind(self.id.tenant_id())
        .bind(id)
        .fetch_optional(&self.state.pool)
        .await? else {
            return Ok(None);
        };

        Ok(Some(self.template_from_row(&row)?))
    }

    async fn create_template(&self, template: runtime_ir::CreateTemplate) -> Result<String, khronos_runtime::Error> {
             let rec = sqlx::query(
            r#"
            INSERT INTO attached_templates (
                owner_type, owner_id, source,
                -- data custom
                name, language, content,
                -- data shop
                shop_ref,
                -- metadata
                created_at, last_updated_at, allowed_caps, state
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW(), $8, $9)
             RETURNING id
            "#,
        )
        .bind(self.id.tenant_type())
        .bind(self.id.tenant_id())
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Custom { .. } => "custom",
            runtime_ir::CreateTemplateSource::Shop { .. } => "shop",
        })
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Custom { name, .. } => Some(name),
            _ => None,
        })
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Custom { language, .. } => Some(language),
            _ => None,
        })
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Custom { content, .. } => Some(serde_json::to_value(content)?),
            _ => None,
        })
        .bind(match &template.source {
            // TODO: More validation here would be useful
            runtime_ir::CreateTemplateSource::Shop { shop_listing } => Some(shop_listing),
            _ => None,
        })
        .bind(&template.allowed_caps)
        .bind(if template.paused { "paused" } else { "active" })
        .fetch_one(&self.state.pool)
        .await?;   
        let id: sqlx::types::Uuid = rec.try_get("id")?;
        Ok(id.to_string())
    }

    async fn update_template(&self, id: &str, template: runtime_ir::CreateTemplate) -> Result<(), khronos_runtime::Error> {
        let id: sqlx::types::Uuid = id.parse().map_err(|_| "Invalid template ID")?;
        sqlx::query(
            r#"
            UPDATE attached_templates
            SET 
                source = $1,
                -- data custom
                name = $2, language = $3, content = $4,
                -- data shop
                shop_ref = $5,
                -- metadata
                last_updated_at = NOW(), allowed_caps = $6, state = $7
            WHERE id = $8 AND owner_type = $9 AND owner_id = $10
            "#,
        )
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Custom { .. } => "custom",
            runtime_ir::CreateTemplateSource::Shop { .. } => "shop",
        })
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Custom { name, .. } => Some(name),
            _ => None,
        })
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Custom { language, .. } => Some(language),
            _ => None,
        })
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Custom { content, .. } => Some(serde_json::to_value(content)?),
            _ => None,
        })
        .bind(match &template.source {
            runtime_ir::CreateTemplateSource::Shop { shop_listing } => Some(shop_listing),
            _ => None,
        })
        .bind(&template.allowed_caps)
        .bind(if template.paused { "paused" } else { "active" })
        .bind(id)
        .bind(self.id.tenant_type())
        .bind(self.id.tenant_id())
        .execute(&self.state.pool)
        .await?;
        Ok(())
    }

    async fn delete_template(&self, id: &str) -> Result<(), khronos_runtime::Error> {
        let id: sqlx::types::Uuid = id.parse().map_err(|_| "Invalid template ID")?;
        sqlx::query(
            r#"DELETE FROM attached_templates 
            WHERE id = $1 AND owner_type = $2 AND owner_id = $3"#,
        )
        .bind(id)
        .bind(self.id.tenant_type())
        .bind(self.id.tenant_id())
        .execute(&self.state.pool)
        .await?;
        Ok(())
    }

    async fn stats(&self) -> Result<runtime_ir::RuntimeStats, khronos_runtime::Error> {
        let sandwich_resp =
        crate::sandwich::get_status(&self.state.reqwest_client).await?;

        let total_guilds = {
            let mut guild_count: i64 = 0;
            sandwich_resp.shard_conns.iter().for_each(|(_, sc)| {
                guild_count += sc.guilds;
            });

            guild_count
        };

        Ok(runtime_ir::RuntimeStats {
            total_cached_guilds: total_guilds.try_into()?, // This field is deprecated, use total_guilds instead
            total_guilds: total_guilds.try_into()?,
            total_users: sandwich_resp.user_count.try_into()?,
            //total_members: sandwich_resp.total_members.try_into()?,
            last_started_at: crate::CONFIG.start_time,
        })
    }

    fn event_list(&self) -> Result<Vec<String>, khronos_runtime::Error> {
        let mut vec = AntiraidEvent::variant_names()
            .iter()
            .map(|x| x.to_string())
            .collect::<Vec<String>>();

        vec.extend(
            dapi::EVENT_LIST
                .iter()
                .copied()
                .map(|x| x.to_string())
                .collect::<Vec<String>>(),
        );

        Ok(vec)
    }

    fn links(&self) -> Result<runtime_ir::RuntimeLinks, khronos_runtime::Error> {
        let support_server = crate::CONFIG.meta.support_server_invite.clone();
        let api_url = crate::CONFIG.sites.api.clone();
        let frontend_url = crate::CONFIG.sites.frontend.clone();
        let docs_url = crate::CONFIG.sites.docs.clone();

        Ok(runtime_ir::RuntimeLinks {
            support_server,
            api_url,
            frontend_url,
            docs_url,
        })
    }

    async fn get_tenant_state(&self) -> Result<runtime_ir::TenantState, khronos_runtime::Error> {
        let ts = self.state.get_cached_tenant_state_for(self.id)?;
        Ok(runtime_ir::TenantState {
            events: ts.events.iter().cloned().collect(),
            banned: ts.banned,
            flags: ts.flags.try_into().unwrap_or(0),
            startup_events: ts.startup_events,
        })
    }

    async fn set_tenant_state(&self, state: runtime_ir::TenantState) -> Result<(), khronos_runtime::Error> {
        self.state
            .set_tenant_state_for(
                self.id,
                TenantState {
                    events: HashSet::from_iter(state.events),
                    banned: state.banned,
                    flags: state.flags.try_into()?,
                    startup_events: state.startup_events,
                },
            )
            .await?;
        Ok(())
    }
}