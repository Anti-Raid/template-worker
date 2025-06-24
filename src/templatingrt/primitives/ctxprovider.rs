use super::{kittycat_permission_config_data, sandwich_config};
use crate::templatingrt::state::GuildState;
use crate::templatingrt::template::Template;
use antiraid_types::userinfo::UserInfo;
use botox::crypto::gen_random;
use khronos_runtime::traits::context::{CompatibilityFlags, KhronosContext, ScriptData};
use khronos_runtime::traits::datastoreprovider::{DataStoreImpl, DataStoreProvider};
use khronos_runtime::traits::discordprovider::DiscordProvider;
use khronos_runtime::traits::ir::kv::KvRecord;
use khronos_runtime::traits::ir::ObjectMetadata;
use khronos_runtime::traits::kvprovider::KVProvider;
use khronos_runtime::traits::lockdownprovider::LockdownProvider;
use khronos_runtime::traits::objectstorageprovider::ObjectStorageProvider;
use khronos_runtime::traits::pageprovider::PageProvider;
use khronos_runtime::traits::userinfoprovider::UserInfoProvider;
use khronos_runtime::utils::khronos_value::KhronosValue;
use moka::future::Cache;
use silverpelt::lockdowns::LockdownData;
use silverpelt::userinfo::{NoMember, UserInfoOperations};
use sqlx::Row;
use std::sync::LazyLock;
use std::{rc::Rc, sync::Arc};

/// Internal short-lived channel cache
pub static CHANNEL_CACHE: LazyLock<Cache<serenity::all::ChannelId, serenity::all::GuildChannel>> =
    LazyLock::new(|| {
        Cache::builder()
            .time_to_idle(std::time::Duration::from_secs(30))
            .build()
    });

#[derive(Clone)]
pub struct TemplateContextProvider {
    guild_state: Rc<GuildState>,

    /// The template data
    template_data: Arc<Template>,

    /// The datastores to expose
    datastores: Vec<Rc<dyn DataStoreImpl>>,

    /// Script data
    script_data: Arc<ScriptData>,
}

impl TemplateContextProvider {
    fn datastores(
        guild_state: Rc<GuildState>,
        _template_data: Arc<Template>,
    ) -> Vec<Rc<dyn DataStoreImpl>> {
        vec![
            Rc::new(super::datastores::StatsStore {
                guild_state: guild_state.clone(),
            }),
            Rc::new(super::datastores::LinksStore {}),
            Rc::new(khronos_runtime::traits::ir::datastores::CopyDataStore {}),
            Rc::new(super::datastores::JobServerStore {
                guild_state: guild_state.clone(),
            }),
        ]
    }

    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(guild_state: Rc<GuildState>, template_data: Arc<Template>) -> Self {
        Self {
            datastores: Self::datastores(guild_state.clone(), template_data.clone()),
            guild_state,
            script_data: Arc::new(ScriptData {
                guild_id: Some(template_data.guild_id),
                name: template_data.name.clone(),
                description: template_data.description.clone(),
                shop_name: template_data.shop_name.clone(),
                shop_owner: template_data.shop_owner,
                events: template_data.events.clone(),
                error_channel: template_data.error_channel,
                lang: template_data.lang.to_string(),
                allowed_caps: template_data.allowed_caps.clone(),
                created_by: Some(template_data.created_by),
                created_at: Some(template_data.created_at),
                updated_by: Some(template_data.updated_by),
                updated_at: Some(template_data.updated_at),
                compatibility_flags: CompatibilityFlags::empty(),
            }),
            template_data,
        }
    }
}

impl KhronosContext for TemplateContextProvider {
    type KVProvider = ArKVProvider;
    type DiscordProvider = ArDiscordProvider;
    type LockdownDataStore = LockdownData;
    type LockdownProvider = ArLockdownProvider;
    type UserInfoProvider = ArUserInfoProvider;
    type PageProvider = ArPageProvider;
    type DataStoreProvider = ArDataStoreProvider;
    type ObjectStorageProvider = ArObjectStorageProvider;

    fn data(&self) -> &ScriptData {
        &self.script_data
    }

    fn allowed_caps(&self) -> &[String] {
        self.template_data.allowed_caps.as_ref()
    }

    /// Returns if the current context has a specific capability
    fn has_cap(&self, cap: &str) -> bool {
        for allowed_cap in self.template_data.allowed_caps.iter() {
            if allowed_cap == cap
                || (allowed_cap == "*" && cap != "assetmanager:use_bundled_templating_types")
            {
                return true;
            }
        }

        false
    }

    fn guild_id(&self) -> Option<serenity::all::GuildId> {
        Some(self.guild_state.guild_id)
    }

    fn owner_guild_id(&self) -> Option<serenity::all::GuildId> {
        self.template_data.shop_owner
    }

    fn template_name(&self) -> String {
        self.template_data.name.clone()
    }

    fn current_user(&self) -> Option<serenity::all::CurrentUser> {
        Some(
            self.guild_state
                .serenity_context
                .cache
                .current_user()
                .clone(),
        )
    }

    fn kv_provider(&self) -> Option<Self::KVProvider> {
        Some(ArKVProvider {
            guild_id: self.guild_state.guild_id,
            guild_state: self.guild_state.clone(),
        })
    }

    fn discord_provider(&self) -> Option<Self::DiscordProvider> {
        Some(ArDiscordProvider {
            guild_id: self.guild_state.guild_id,
            guild_state: self.guild_state.clone(),
        })
    }

    fn lockdown_provider(&self) -> Option<Self::LockdownProvider> {
        Some(ArLockdownProvider {
            guild_state: self.guild_state.clone(),
            lockdown_data: Rc::new(LockdownData::new(
                self.guild_state.serenity_context.cache.clone(),
                self.guild_state.serenity_context.http.clone(),
                self.guild_state.pool.clone(),
                self.guild_state.reqwest_client.clone(),
                sandwich_config(),
            )),
        })
    }

    fn userinfo_provider(&self) -> Option<Self::UserInfoProvider> {
        Some(ArUserInfoProvider {
            guild_id: self.guild_state.guild_id,
            guild_state: self.guild_state.clone(),
        })
    }

    fn page_provider(&self) -> Option<Self::PageProvider> {
        Some(ArPageProvider {
            guild_id: self.guild_state.guild_id,
            guild_state: self.guild_state.clone(),
            template_id: self.template_data.name.clone(),
        })
    }

    fn datastore_provider(&self) -> Option<Self::DataStoreProvider> {
        Some(ArDataStoreProvider {
            guild_id: self.guild_state.guild_id,
            guild_state: self.guild_state.clone(),
            datastores: self.datastores.clone(),
        })
    }

    fn objectstorage_provider(&self) -> Option<Self::ObjectStorageProvider> {
        Some(ArObjectStorageProvider {
            guild_id: self.guild_state.guild_id,
            guild_state: self.guild_state.clone(),
        })
    }
}

#[derive(Clone)]
pub struct ArKVProvider {
    guild_id: serenity::all::GuildId,
    guild_state: Rc<GuildState>,
}

impl KVProvider for ArKVProvider {
    fn attempt_action(&self, _scope: &[String], bucket: &str) -> Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.kv.check(bucket)
    }

    async fn get(
        &self,
        scopes: &[String],
        key: String,
    ) -> Result<Option<KvRecord>, silverpelt::Error> {
        // Check key length
        if key.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let rec = sqlx::query(
            "SELECT id, expires_at, scopes, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3",
        )
        .bind(self.guild_id.to_string())
        .bind(&key)
        .bind(scopes)
        .fetch_optional(&self.guild_state.pool)
        .await?;

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

    async fn get_by_id(&self, id: String) -> Result<Option<KvRecord>, silverpelt::Error> {
        let rec = sqlx::query(
            "SELECT key, expires_at, scopes, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND id = $2",
        )
        .bind(self.guild_id.to_string())
        .bind(id.clone())
        .fetch_optional(&self.guild_state.pool)
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

    async fn list_scopes(&self) -> Result<Vec<String>, silverpelt::Error> {
        let rec = sqlx::query(
            "SELECT DISTINCT unnest_scope AS scope
FROM guild_templates_kv, unnest(scopes) AS unnest_scope
ORDER BY scope",
        )
        .bind(self.guild_id.to_string())
        .fetch_all(&self.guild_state.pool)
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
    ) -> Result<(bool, String), silverpelt::Error> {
        // Check key length
        if key.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        // Check bytes length
        let data_str = serde_json::to_string(&data)?;

        if data_str.len() > self.guild_state.kv_constraints.max_value_bytes {
            return Err("Value length too long".into());
        }

        let mut tx = self.guild_state.pool.begin().await?;

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
            crate::templatingrt::cache::get_all_guild_key_expiries_from_db(
                self.guild_state.guild_id,
                &self.guild_state.pool,
            )
            .await?;
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
        if key.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        sqlx::query(
            "UPDATE guild_templates_kv SET expires_at = $1, last_updated_at = NOW() WHERE guild_id = $2 AND key = $3 AND scopes @> $4",
        )
        .bind(expires_at)
        .bind(self.guild_id.to_string())
        .bind(&key)
        .bind(scopes)
        .execute(&self.guild_state.pool)
        .await?;

        // Regenerate the cache in any case
        crate::templatingrt::cache::get_all_guild_key_expiries_from_db(
            self.guild_state.guild_id,
            &self.guild_state.pool,
        )
        .await?;

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
        .execute(&self.guild_state.pool)
        .await?;

        // Regenerate the cache in any case
        crate::templatingrt::cache::get_all_guild_key_expiries_from_db(
            self.guild_state.guild_id,
            &self.guild_state.pool,
        )
        .await?;

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

        if data_str.len() > self.guild_state.kv_constraints.max_value_bytes {
            return Err("Value length too long".into());
        }

        let mut tx = self.guild_state.pool.begin().await?;

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
            crate::templatingrt::cache::get_all_guild_key_expiries_from_db(
                self.guild_state.guild_id,
                &self.guild_state.pool,
            )
            .await?;
        }

        Ok(())
    }

    async fn delete(&self, scopes: &[String], key: String) -> Result<(), silverpelt::Error> {
        // Check key length
        if key.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let row = sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3 RETURNING expires_at",
        )
        .bind(self.guild_id.to_string())
        .bind(key)
        .bind(scopes)
        .fetch_optional(&self.guild_state.pool)
        .await?;

        if let Some(row) = row {
            let expires_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("expires_at")?;

            if expires_at.is_some() {
                // Regenerate the cache if the key has an expiry set
                crate::templatingrt::cache::get_all_guild_key_expiries_from_db(
                    self.guild_state.guild_id,
                    &self.guild_state.pool,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn delete_by_id(&self, id: String) -> Result<(), silverpelt::Error> {
        let row = sqlx::query(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND id = $2 RETURNING expires_at",
        )
        .bind(self.guild_id.to_string())
        .bind(id)
        .fetch_optional(&self.guild_state.pool)
        .await?;

        if let Some(row) = row {
            let expires_at: Option<chrono::DateTime<chrono::Utc>> = row.try_get("expires_at")?;

            if expires_at.is_some() {
                // Regenerate the cache if the key has an expiry set
                crate::templatingrt::cache::get_all_guild_key_expiries_from_db(
                    self.guild_state.guild_id,
                    &self.guild_state.pool,
                )
                .await?;
            }
        }

        Ok(())
    }

    async fn find(
        &self,
        scopes: &[String],
        query: String,
    ) -> Result<Vec<KvRecord>, silverpelt::Error> {
        // Check key length
        if query.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Query length too long".into());
        }

        let rec = {
            if query == "%%" {
                // Fast path, omit ILIKE if '%%' is used
                sqlx::query(
                    "SELECT id, key, value, expires_at, created_at, last_updated_at, scopes FROM guild_templates_kv WHERE guild_id = $1 AND scopes @> $2",
                )
                .bind(self.guild_id.to_string())
                .bind(scopes)
                .fetch_all(&self.guild_state.pool)
                .await?
            } else {
                sqlx::query(
                    "SELECT id, key, value, expires_at, created_at, last_updated_at, scopes FROM guild_templates_kv WHERE guild_id = $1 AND scopes @> $2 AND key ILIKE $3",
                )
                .bind(self.guild_id.to_string())
                .bind(scopes)
                .bind(query)
                .fetch_all(&self.guild_state.pool)
                .await?
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

    async fn exists(&self, scopes: &[String], key: String) -> Result<bool, silverpelt::Error> {
        // Check key length
        if key.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let rec = sqlx::query(
            "SELECT id FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scopes @> $3",
        )
        .bind(self.guild_id.to_string())
        .bind(key)
        .bind(scopes)
        .fetch_optional(&self.guild_state.pool)
        .await?
        .is_some();

        Ok(rec)
    }

    async fn keys(&self, scopes: &[String]) -> Result<Vec<String>, silverpelt::Error> {
        let rec =
            sqlx::query("SELECT key FROM guild_templates_kv WHERE guild_id = $1 AND scopes @> $2")
                .bind(self.guild_id.to_string())
                .bind(scopes)
                .fetch_all(&self.guild_state.pool)
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
    guild_state: Rc<GuildState>,
}

impl DiscordProvider for ArDiscordProvider {
    fn attempt_action(&self, bucket: &str) -> serenity::Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.discord.check(bucket)
    }

    async fn get_guild(
        &self,
    ) -> serenity::Result<serenity::model::prelude::PartialGuild, silverpelt::Error> {
        Ok(sandwich_driver::guild(
            &self.guild_state.serenity_context.cache,
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            self.guild_id,
            &sandwich_config(),
        )
        .await
        .map_err(|e| format!("Failed to fetch guild information from sandwich: {}", e))?)
    }

    async fn get_guild_member(
        &self,
        user_id: serenity::all::UserId,
    ) -> serenity::Result<Option<serenity::all::Member>, silverpelt::Error> {
        Ok(sandwich_driver::member_in_guild(
            &self.guild_state.serenity_context.cache,
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            self.guild_id,
            user_id,
            &sandwich_config(),
        )
        .await
        .map_err(|e| format!("Failed to fetch member information from sandwich: {}", e))?)
    }

    async fn get_channel(
        &self,
        channel_id: serenity::all::ChannelId,
    ) -> serenity::Result<serenity::all::GuildChannel, silverpelt::Error> {
        {
            // Check cache first
            let cached_channel = CHANNEL_CACHE.get(&channel_id).await;

            if let Some(cached_channel) = cached_channel {
                if cached_channel.guild_id != self.guild_id {
                    return Err("Channel not in guild".into());
                }

                return Ok(cached_channel);
            }
        }

        let channel = sandwich_driver::channel(
            &self.guild_state.serenity_context.cache,
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            Some(self.guild_id),
            channel_id,
            &sandwich_config(),
        )
        .await
        .map_err(|e| format!("Failed to fetch channel information from sandwich: {}", e))?;

        let Some(channel) = channel else {
            return Err("Channel not found".into());
        };

        let Some(guild_channel) = channel.guild() else {
            return Err("Channel not in guild".into());
        };

        if guild_channel.guild_id != self.guild_id {
            return Err("Channel not in guild".into());
        }

        Ok(guild_channel)
    }

    fn guild_id(&self) -> serenity::all::GuildId {
        self.guild_id
    }

    fn serenity_http(&self) -> &serenity::http::Http {
        &self.guild_state.serenity_context.http
    }

    async fn edit_channel_permissions(
        &self,
        channel_id: serenity::all::ChannelId,
        target_id: serenity::all::TargetId,
        data: impl serde::Serialize,
        audit_log_reason: Option<&str>,
    ) -> Result<(), khronos_runtime::Error> {
        self.guild_state
            .serenity_context
            .http
            .create_permission(channel_id, target_id, &data, audit_log_reason)
            .await
            .map_err(|e| format!("Failed to edit channel permissions: {}", e))?;

        // Update cache
        CHANNEL_CACHE.remove(&channel_id).await;

        Ok(())
    }

    async fn edit_channel(
        &self,
        channel_id: serenity::all::ChannelId,
        map: impl serde::Serialize,
        audit_log_reason: Option<&str>,
    ) -> Result<serenity::model::channel::GuildChannel, silverpelt::Error> {
        let chan = self
            .guild_state
            .serenity_context
            .http
            .edit_channel(channel_id, &map, audit_log_reason)
            .await
            .map_err(|e| format!("Failed to edit channel: {}", e))?;

        // Update cache
        CHANNEL_CACHE.insert(channel_id, chan.clone()).await;

        Ok(chan)
    }

    async fn delete_channel(
        &self,
        channel_id: serenity::all::ChannelId,
        audit_log_reason: Option<&str>,
    ) -> Result<serenity::model::channel::Channel, silverpelt::Error> {
        let chan = self
            .guild_state
            .serenity_context
            .http
            .delete_channel(channel_id, audit_log_reason)
            .await
            .map_err(|e| format!("Failed to delete channel: {}", e))?;

        // Remove from cache
        CHANNEL_CACHE.remove(&channel_id).await;

        Ok(chan)
    }
}

#[derive(Clone)]
pub struct ArLockdownProvider {
    guild_state: Rc<GuildState>,
    lockdown_data: Rc<LockdownData>,
}

impl LockdownProvider<LockdownData> for ArLockdownProvider {
    fn attempt_action(&self, bucket: &str) -> serenity::Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.lockdowns.check(bucket)
    }

    /// Returns a lockdown data store to be used with the lockdown library
    fn lockdown_data_store(&self) -> &LockdownData {
        &self.lockdown_data
    }

    /// Serenity HTTP client
    fn serenity_http(&self) -> &serenity::http::Http {
        &self.guild_state.serenity_context.http
    }
}

#[derive(Clone)]
pub struct ArUserInfoProvider {
    guild_id: serenity::all::GuildId,
    guild_state: Rc<GuildState>,
}

impl UserInfoProvider for ArUserInfoProvider {
    fn attempt_action(&self, bucket: &str) -> serenity::Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.userinfo.check(bucket)
    }

    async fn get(&self, user_id: serenity::all::UserId) -> Result<UserInfo, silverpelt::Error> {
        let userinfo = UserInfo::get(
            self.guild_id,
            user_id,
            &self.guild_state.pool,
            &self.guild_state.serenity_context,
            &self.guild_state.reqwest_client,
            kittycat_permission_config_data(),
            &sandwich_config(),
            None::<NoMember>,
        )
        .await
        .map_err(|e| format!("Failed to get user info: {}", e))?;

        Ok(userinfo)
    }
}

#[derive(Clone)]
pub struct ArPageProvider {
    template_id: String,
    guild_id: serenity::all::GuildId,
    guild_state: Rc<GuildState>,
}

impl PageProvider for ArPageProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.page.check(bucket)
    }

    async fn get_page(&self) -> Option<khronos_runtime::traits::ir::Page> {
        crate::pages::get_page_by_id(self.guild_id, &self.template_id)
            .await
            .map(crate::pages::unravel_page)
    }

    async fn set_page(
        &self,
        page: khronos_runtime::traits::ir::Page,
    ) -> Result<(), khronos_runtime::Error> {
        crate::pages::set_page(
            self.guild_id,
            self.template_id.clone(),
            crate::pages::create_page(page, self.guild_id, self.template_id.clone()),
        )
        .await;

        Ok(())
    }

    async fn delete_page(&self) -> Result<(), khronos_runtime::Error> {
        crate::pages::remove_page(self.guild_id, &self.template_id).await;
        Ok(())
    }
}

#[derive(Clone)]
#[allow(dead_code)]
pub struct ArDataStoreProvider {
    guild_id: serenity::all::GuildId,
    guild_state: Rc<GuildState>,
    datastores: Vec<Rc<dyn DataStoreImpl>>,
}

impl DataStoreProvider for ArDataStoreProvider {
    fn attempt_action(&self, method: &str, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.guild_state
            .ratelimits
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
    guild_state: Rc<GuildState>,
}

impl ObjectStorageProvider for ArObjectStorageProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.guild_state.ratelimits.object_storage.check(bucket)
    }

    fn bucket_name(&self) -> String {
        silverpelt::objectstore::guild_bucket(self.guild_id)
    }

    async fn list_files(
        &self,
        prefix: Option<String>,
    ) -> Result<Vec<ObjectMetadata>, khronos_runtime::Error> {
        Ok(self
            .guild_state
            .object_store
            .list_files(
                &silverpelt::objectstore::guild_bucket(self.guild_id),
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
            .guild_state
            .object_store
            .exists(&silverpelt::objectstore::guild_bucket(self.guild_id), &key)
            .await?)
    }

    async fn download_file(&self, key: String) -> Result<Vec<u8>, khronos_runtime::Error> {
        Ok(self
            .guild_state
            .object_store
            .download_file(&silverpelt::objectstore::guild_bucket(self.guild_id), &key)
            .await?)
    }

    async fn get_file_url(
        &self,
        key: String,
        expiry: std::time::Duration,
    ) -> Result<String, khronos_runtime::Error> {
        Ok(self
            .guild_state
            .object_store
            .get_url(
                &silverpelt::objectstore::guild_bucket(self.guild_id),
                &key,
                expiry,
            )
            .await?)
    }

    async fn upload_file(&self, key: String, data: Vec<u8>) -> Result<(), khronos_runtime::Error> {
        if key.len()
            > self
                .guild_state
                .kv_constraints
                .max_object_storage_path_length
        {
            return Err("Path length too long".into());
        }

        if data.len() > self.guild_state.kv_constraints.max_object_storage_bytes {
            return Err("Data too large".into());
        }

        self.guild_state
            .object_store
            .upload_file(
                &silverpelt::objectstore::guild_bucket(self.guild_id),
                &key,
                data,
            )
            .await?;

        Ok(())
    }

    async fn delete_file(&self, key: String) -> Result<(), khronos_runtime::Error> {
        Ok(self
            .guild_state
            .object_store
            .delete(&silverpelt::objectstore::guild_bucket(self.guild_id), &key)
            .await?)
    }
}
