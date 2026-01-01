use super::workerstate::WorkerState;
use super::workervmmanager::Id;
use crate::events::AntiraidEvent;
use crate::objectstore::{Bucket, BucketWithKey, BucketWithPrefix};
use crate::worker::builtins::EXPOSED_VFS;
use crate::worker::workerstate::TenantState;
use crate::worker::workervmmanager::VmData;
use khronos_runtime::core::typesext::Vfs;
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
use std::rc::Rc;
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
}

impl TemplateContextProvider {
    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(
        id: Id,
        vm_data: VmData,
    ) -> Self {
        Self {
            id,
            state: vm_data.state,
            kv_constraints: vm_data.kv_constraints,
            ratelimits: vm_data.ratelimits,
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
            bucket: Bucket::Guild(self.guild_id()?),
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
}

const MAX_SCOPES: usize = 10;
impl ArKVProvider {
    fn parse_scopes(scopes: &[String]) -> Result<Vec<String>, crate::Error> {        
        if scopes.len() > MAX_SCOPES {
            return Err(format!("Scopes length may be at most {MAX_SCOPES} long").into())
        }
        let mut scopes = scopes.to_vec();
        scopes.sort();
        Ok(scopes)
    }
}

impl KVProvider for ArKVProvider {
    fn attempt_action(&self, _scope: &[String], bucket: &str) -> Result<(), crate::Error> {
        self.ratelimits.kv.check(bucket)
    }

    async fn get(&self, scopes: &[String], key: String) -> Result<Option<KvRecord>, crate::Error> {
        let scopes = Self::parse_scopes(scopes)?;

        // Check key length
        if key.len() > self.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let rec = if scopes.is_empty() {
            sqlx::query(
            "SELECT id, scopes, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
            )
            .bind(self.guild_id.to_string())
            .bind(&key)
            .fetch_optional(&self.state.pool)
            .await?
        } else {
            sqlx::query(
            "SELECT id, scopes, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3",
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
        }))
    }

    async fn get_by_id(&self, id: String) -> Result<Option<KvRecord>, crate::Error> {
        let rec = sqlx::query(
            "SELECT key, scopes, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND id = $2",
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
    ) -> Result<(bool, String), crate::Error> {
        let scopes = Self::parse_scopes(scopes)?;

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

        let id = gen_random(64);
        let rec = sqlx::query(
            "INSERT INTO guild_templates_kv (id, guild_id, key, value, scopes) VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (guild_id, key, scopes) DO UPDATE value = EXCLUDED.value, last_updated = NOW()
            RETURNING id, (old.id IS NOT NULL) as exists",
        )
        .bind(&id)
        .bind(self.guild_id.to_string())
        .bind(key)
        .bind(serde_json::to_value(data)?)
        .bind(scopes)
        .fetch_one(&self.state.pool)
        .await?;

        let id = rec.try_get("id")?;
        let exists = rec.try_get("exists")?;

        Ok((exists, id))
    }

    async fn set_by_id(
        &self,
        id: String,
        data: KhronosValue,
    ) -> Result<(), khronos_runtime::Error> {
        // Check bytes length
        let data_str = serde_json::to_string(&data)?;

        if data_str.len() > self.kv_constraints.max_value_bytes {
            return Err("Value length too long".into());
        }

        // Update existing record
        sqlx::query(
            "UPDATE guild_templates_kv SET value = $1, last_updated_at = NOW() WHERE id = $2",
        )
        .bind(serde_json::to_value(data)?)
        .bind(&id)
        .execute(&self.state.pool)
        .await?;

        Ok(())
    }

    async fn delete(&self, scopes: &[String], key: String) -> Result<(), crate::Error> {
        // Check key length
        if key.len() > self.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        if scopes.is_empty() {
            sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
            )
            .bind(self.guild_id.to_string())
            .bind(key)
            .execute(&self.state.pool)
            .await?;
        } else {
            sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3",
            )
            .bind(self.guild_id.to_string())
            .bind(key)
            .bind(scopes)
            .execute(&self.state.pool)
            .await?;
        };

        Ok(())
    }

    async fn delete_by_id(&self, id: String) -> Result<(), crate::Error> {
        sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND id = $2",
        )
        .bind(self.guild_id.to_string())
        .bind(id)
        .execute(&self.state.pool)
        .await?;

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
                    "SELECT id, key, value, created_at, last_updated_at, scopes, resume FROM guild_templates_kv WHERE guild_id = $1",
                    )
                    .bind(self.guild_id.to_string())
                    .fetch_all(&self.state.pool)
                    .await?
                } else {
                    // no query, scopes
                    sqlx::query(
                    "SELECT id, key, value, created_at, last_updated_at, scopes, resume FROM guild_templates_kv WHERE guild_id = $1 AND scopes @> $2",
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
                    "SELECT id, key, value, created_at, last_updated_at, scopes, resume FROM guild_templates_kv WHERE guild_id = $1 AND key ILIKE $2",
                    )
                    .bind(self.guild_id.to_string())
                    .bind(query)
                    .fetch_all(&self.state.pool)
                    .await?
                } else {
                    // query, scopes
                    sqlx::query(
                    "SELECT id, key, value, created_at, last_updated_at, scopes, resume FROM guild_templates_kv WHERE guild_id = $1 AND scopes @> $2 AND key ILIKE $3",
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
    bucket: Bucket,
    ratelimits: Rc<Ratelimits>,
    state: WorkerState,
    kv_constraints: LuaKVConstraints,
}

impl ObjectStorageProvider for ArObjectStorageProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.ratelimits.object_storage.check(bucket)
    }

    fn bucket_name(&self) -> String {
        self.bucket.prefix()
    }

    async fn list_files(
        &self,
        prefix: Option<String>,
    ) -> Result<Vec<ObjectMetadata>, khronos_runtime::Error> {
        Ok(self
            .state
            .object_store
            .list_files(
                BucketWithPrefix::new(self.bucket, prefix.as_deref())
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
            .exists(BucketWithKey::new(self.bucket, &key))
            .await?)
    }

    async fn download_file(&self, key: String) -> Result<Vec<u8>, khronos_runtime::Error> {
        Ok(self
            .state
            .object_store
            .download_file(BucketWithKey::new(self.bucket, &key))
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
                BucketWithKey::new(self.bucket, &key),
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
            .upload_file(BucketWithKey::new(self.bucket, &key), data)
            .await?;

        Ok(())
    }

    async fn delete_file(&self, key: String) -> Result<(), khronos_runtime::Error> {
        Ok(self
            .state
            .object_store
            .delete(BucketWithKey::new(self.bucket, &key))
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

impl RuntimeProvider for ArRuntimeProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.ratelimits.runtime.check(bucket)
    }

    fn get_exposed_vfs(&self) -> Result<std::collections::HashMap<String, Vfs>, khronos_runtime::Error> {
        Ok((&*EXPOSED_VFS).clone())
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
        let ts = self.state.get_cached_tenant_state_for(self.id)?.into_owned();
        Ok(runtime_ir::TenantState {
            events: ts.events.into_iter().collect(),
            banned: ts.banned,
            data: ts.data,
        })
    }

    async fn set_tenant_state(&self, state: runtime_ir::TenantState) -> Result<(), khronos_runtime::Error> {
        self.state
            .set_tenant_state_for(
                self.id,
                TenantState {
                    events: HashSet::from_iter(state.events),
                    banned: state.banned,
                    data: state.data,
                },
            )
            .await?;
        Ok(())
    }
}