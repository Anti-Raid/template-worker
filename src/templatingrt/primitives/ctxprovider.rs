use crate::templatingrt::state::GuildState;
use crate::templatingrt::template::Template;
use antiraid_types::stings::{Sting, StingAggregate};
use antiraid_types::userinfo::UserInfo;
use khronos_runtime::traits::context::KhronosContext;
use khronos_runtime::traits::discordprovider::DiscordProvider;
use khronos_runtime::traits::ir::kv::KvRecord;
use khronos_runtime::traits::kvprovider::KVProvider;
use khronos_runtime::traits::lockdownprovider::LockdownProvider;
use khronos_runtime::traits::pageprovider::PageProvider;
use khronos_runtime::traits::stingprovider::StingProvider;
use khronos_runtime::traits::userinfoprovider::UserInfoProvider;
use khronos_runtime::utils::executorscope::ExecutorScope;
use moka::future::Cache;
use silverpelt::lockdowns::LockdownData;
use silverpelt::stings::{StingAggregateOperations, StingCreateOperations, StingOperations};
use silverpelt::userinfo::{NoMember, UserInfoOperations};
use sqlx::Row;
use std::sync::LazyLock;
use std::{rc::Rc, sync::Arc};
use super::{kittycat_permission_config_data, sandwich_config};

/// Internal short-lived channel cache
pub static CHANNEL_CACHE: LazyLock<Cache<serenity::all::ChannelId, serenity::all::GuildChannel>> =
    LazyLock::new(|| {
        Cache::builder()
            .time_to_idle(std::time::Duration::from_secs(30))
            .build()
    });

#[derive(Clone)]
pub struct TemplateContextProvider {
    pub guild_state: Rc<GuildState>,

    /// The template data
    pub template_data: Arc<Template>,

    /// The isolate being used
    pub runtime_shareable_data: khronos_runtime::rt::RuntimeShareableData
}

impl KhronosContext for TemplateContextProvider {
    type Data = Arc<Template>;
    type KVProvider = ArKVProvider;
    type DiscordProvider = ArDiscordProvider;
    type LockdownDataStore = LockdownData;
    type LockdownProvider = ArLockdownProvider;
    type UserInfoProvider = ArUserInfoProvider;
    type StingProvider = ArStingProvider;
    type PageProvider = ArPageProvider;

    fn data(&self) -> Self::Data {
        self.template_data.clone()
    }

    fn allowed_caps(&self) -> &[String] {
        self.template_data.allowed_caps.as_ref()
    }

    fn has_cap(&self, cap: &str) -> bool {
        self.template_data.allowed_caps.contains(&cap.to_string())
    }

    fn guild_id(&self) -> Option<serenity::all::GuildId> {
        Some(self.guild_state.guild_id)
    }

    fn owner_guild_id(&self) -> Option<serenity::all::GuildId> {
        self.template_data.shop_owner
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

    fn kv_provider(&self, scope: ExecutorScope) -> Option<Self::KVProvider> {
        Some(ArKVProvider {
            guild_id: match scope {
                ExecutorScope::ThisGuild => self.guild_state.guild_id,
                ExecutorScope::OwnerGuild => self
                    .template_data
                    .shop_owner
                    .unwrap_or(self.guild_state.guild_id),
            },
            guild_state: self.guild_state.clone(),
        })
    }

    fn discord_provider(&self, scope: ExecutorScope) -> Option<Self::DiscordProvider> {
        Some(ArDiscordProvider {
            guild_id: match scope {
                ExecutorScope::ThisGuild => self.guild_state.guild_id,
                ExecutorScope::OwnerGuild => self
                    .template_data
                    .shop_owner
                    .unwrap_or(self.guild_state.guild_id), // TODO: consider if we should support ownerguild scope here
            },
            guild_state: self.guild_state.clone(),
        })
    }

    fn lockdown_provider(&self, _scope: ExecutorScope) -> Option<Self::LockdownProvider> {
        Some(ArLockdownProvider {
            guild_state: self.guild_state.clone(),
            lockdown_data: Rc::new(
                LockdownData::new(
                    self.guild_state.serenity_context.cache.clone(),
                    self.guild_state.serenity_context.http.clone(),
                    self.guild_state.pool.clone(),
                    self.guild_state.reqwest_client.clone(),
                    sandwich_config(),
                )
            )
        })
    }

    fn userinfo_provider(&self, scope: ExecutorScope) -> Option<Self::UserInfoProvider> {
        Some(ArUserInfoProvider {
            guild_id: match scope {
                ExecutorScope::ThisGuild => self.guild_state.guild_id,
                ExecutorScope::OwnerGuild => self
                    .template_data
                    .shop_owner
                    .unwrap_or(self.guild_state.guild_id),
            },
            guild_state: self.guild_state.clone(),
        })
    }

    fn sting_provider(&self, scope: ExecutorScope) -> Option<Self::StingProvider> {
        Some(ArStingProvider {
            guild_id: match scope {
                ExecutorScope::ThisGuild => self.guild_state.guild_id,
                ExecutorScope::OwnerGuild => self
                    .template_data
                    .shop_owner
                    .unwrap_or(self.guild_state.guild_id), // TODO: consider if we should support ownerguild scope here
            },
            guild_state: self.guild_state.clone(),
        })
    }

    fn runtime_shareable_data(&self) -> khronos_runtime::rt::RuntimeShareableData {
        self.runtime_shareable_data.clone()
    }

    fn page_provider(&self, scope: ExecutorScope) -> Option<Self::PageProvider> {
        Some(ArPageProvider {
            guild_id: match scope {
                ExecutorScope::ThisGuild => self.guild_state.guild_id,
                ExecutorScope::OwnerGuild => self
                    .template_data
                    .shop_owner
                    .unwrap_or(self.guild_state.guild_id), // TODO: consider if we should support ownerguild scope here
            },
            guild_state: self.guild_state.clone(),
            template_id: self.template_data.name.clone(),
        })
    }
}

#[derive(Clone)]
pub struct ArKVProvider {
    guild_id: serenity::all::GuildId,
    guild_state: Rc<GuildState>,
}

impl KVProvider for ArKVProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.kv.check(bucket)
    }

    async fn get(&self, key: String) -> Result<Option<KvRecord>, silverpelt::Error> {
        // Check key length
        if key.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let rec = sqlx::query(
            "SELECT value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
        )
        .bind(self.guild_id.to_string())
        .bind(&key)
        .fetch_optional(&self.guild_state.pool)
        .await?;

        let Some(rec) = rec else {
            return Ok(None);
        };

        Ok(Some(KvRecord {
            key,
            value: rec
                .try_get::<Option<serde_json::Value>, _>("value")?
                .unwrap_or(serde_json::Value::Null),
            created_at: Some(rec.try_get("created_at")?),
            last_updated_at: Some(rec.try_get("last_updated_at")?),
        }))
    }

    async fn set(&self, key: String, data: serde_json::Value) -> Result<(), silverpelt::Error> {
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

        let rec: i64 = sqlx::query("SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1")
            .bind(self.guild_id.to_string())
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .try_get::<Option<i64>, _>("count")?
            .unwrap_or_default();

        if rec
            >= TryInto::<i64>::try_into(
                silverpelt::templates::LuaKVConstraints::default().max_keys,
            )?
        {
            return Err("Max keys limit reached".into());
        }

        sqlx::query(
            "INSERT INTO guild_templates_kv (guild_id, key, value) VALUES ($1, $2, $3) ON CONFLICT (guild_id, key) DO UPDATE SET value = $3, last_updated_at = NOW()",
        )
        .bind(self.guild_id.to_string())
        .bind(key)
        .bind(data)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(())
    }

    async fn delete(&self, key: String) -> Result<(), silverpelt::Error> {
        // Check key length
        if key.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        sqlx::query("DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2")
            .bind(self.guild_id.to_string())
            .bind(key)
            .execute(&self.guild_state.pool)
            .await?;

        Ok(())
    }

    async fn find(&self, query: String) -> Result<Vec<KvRecord>, silverpelt::Error> {
        // Check key length
        if query.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Query length too long".into());
        }

        let rec = sqlx::query(
            "SELECT key, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key ILIKE $2",
        )
        .bind(self.guild_id.to_string())
        .bind(query)
        .fetch_all(&self.guild_state.pool)
        .await?;

        let mut records = vec![];

        for rec in rec {
            let record = KvRecord {
                key: rec.try_get("key")?,
                value: rec
                    .try_get::<Option<serde_json::Value>, _>("value")?
                    .unwrap_or(serde_json::Value::Null),
                created_at: Some(rec.try_get("created_at")?),
                last_updated_at: Some(rec.try_get("last_updated_at")?),
            };

            records.push(record);
        }

        Ok(records)
    }

    async fn exists(&self, key: String) -> Result<bool, silverpelt::Error> {
        // Check key length
        if key.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Key length too long".into());
        }

        let rec =
            sqlx::query("SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1 AND key = $2")
                .bind(self.guild_id.to_string())
                .bind(key)
                .fetch_one(&self.guild_state.pool)
                .await?
                .try_get::<Option<i64>, _>("count")?
                .unwrap_or_default();

        Ok(rec > 0)
    }

    async fn keys(&self) -> Result<Vec<String>, silverpelt::Error> {
        let rec = sqlx::query("SELECT key FROM guild_templates_kv WHERE guild_id = $1")
            .bind(self.guild_id.to_string())
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

    async fn guild(
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

    async fn member(
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

    async fn guild_channel(
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
pub struct ArStingProvider {
    guild_id: serenity::all::GuildId,
    guild_state: Rc<GuildState>,
}

impl StingProvider for ArStingProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.stings.check(bucket)
    }

    async fn list(&self, page: usize) -> Result<Vec<Sting>, silverpelt::Error> {
        let stings = Sting::list(&self.guild_state.pool, self.guild_id, page).await?;

        Ok(stings)
    }

    async fn get(
        &self,
        id: sqlx::types::uuid::Uuid,
    ) -> Result<Option<antiraid_types::stings::Sting>, silverpelt::Error> {
        let sting = Sting::get(&self.guild_state.pool, self.guild_id, id).await?;

        if let Some(ref pot_sting) = sting {
            if pot_sting.guild_id != self.guild_id {
                return Err("sting not associated with this guild".into());
            }
        }

        Ok(sting)
    }

    async fn create(
        &self,
        sting: antiraid_types::stings::StingCreate,
    ) -> Result<sqlx::types::uuid::Uuid, silverpelt::Error> {
        if sting.guild_id != self.guild_id {
            return Err("stingcreate not associated with this guild".into());
        }

        let sting = sting
            .create_without_dispatch(&self.guild_state.pool)
            .await?;

        Ok(sting.id)
    }

    async fn update(&self, sting: antiraid_types::stings::Sting) -> Result<(), silverpelt::Error> {
        if sting.guild_id != self.guild_id {
            return Err("sting not associated with this guild".into());
        }

        let real_guild_id = Sting::guild_id(sting.id, &self.guild_state.pool)
            .await
            .map_err(|e| format!("failed to fetch guild_id associated with sting: {}", e))?;

        if real_guild_id != sting.guild_id {
            return Err("cannot change guild_id associated with sting".into());
        }

        sting
            .update_without_dispatch(&self.guild_state.pool)
            .await
            .map_err(|e| format!("failed to update sting: {}", e))?;

        Ok(())
    }

    async fn delete(&self, id: sqlx::types::uuid::Uuid) -> Result<(), silverpelt::Error> {
        Sting::delete_without_dispatch(&self.guild_state.pool, self.guild_id, id)
            .await
            .map_err(|e| format!("failed to delete sting: {}", e))?;

        Ok(())
    }

    async fn guild_aggregate(
        &self,
    ) -> Result<Vec<antiraid_types::stings::StingAggregate>, khronos_runtime::Error> {
        let stings = StingAggregate::guild(&self.guild_state.pool, self.guild_id)
            .await
            .map_err(|e| format!("failed to fetch sting aggregate: {}", e))?;

        Ok(stings)
    }

    async fn guild_user_aggregate(
        &self,
        target: serenity::all::UserId,
    ) -> Result<Vec<StingAggregate>, khronos_runtime::Error> {
        let stings = StingAggregate::guild_user(&self.guild_state.pool, self.guild_id, target)
            .await
            .map_err(|e| format!("failed to fetch sting aggregate: {}", e))?;

        Ok(stings)
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
