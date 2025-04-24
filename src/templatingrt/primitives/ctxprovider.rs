use crate::templatingrt::state::GuildState;
use crate::templatingrt::template::Template;
use antiraid_types::userinfo::UserInfo;
use khronos_runtime::traits::context::KhronosContext;
use khronos_runtime::traits::discordprovider::DiscordProvider;
use khronos_runtime::traits::ir::kv::KvRecord;
use khronos_runtime::traits::kvprovider::KVProvider;
use khronos_runtime::traits::lockdownprovider::LockdownProvider;
use khronos_runtime::traits::pageprovider::PageProvider;
use khronos_runtime::traits::userinfoprovider::UserInfoProvider;
use khronos_runtime::traits::scheduledexecprovider::ScheduledExecProvider;
use khronos_runtime::traits::datastoreprovider::{DataStoreProvider, DataStoreImpl};
use khronos_runtime::traits::ir::ScheduledExecution;
use khronos_runtime::utils::executorscope::ExecutorScope;
use moka::future::Cache;
use silverpelt::lockdowns::LockdownData;
use silverpelt::userinfo::{NoMember, UserInfoOperations};
use sqlx::Row;
use std::sync::LazyLock;
use std::{rc::Rc, sync::Arc};
use super::{kittycat_permission_config_data, sandwich_config};
use crate::templatingrt::primitives::assetmanager::TemplateAssetManager;

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

    /// The isolate data being used
    runtime_shareable_data: khronos_runtime::rt::RuntimeShareableData,

    /// The datastores to expose
    datastores: Vec<Rc<dyn DataStoreImpl>>,
}

impl TemplateContextProvider {
    fn datastores(guild_state: Rc<GuildState>, _template_data: Arc<Template>, manager: khronos_runtime::rt::KhronosRuntimeManager<TemplateAssetManager>) -> Vec<Rc<dyn DataStoreImpl>> {
        vec![
            Rc::new(
                super::datastores::StatsStore {
                    guild_state: guild_state.clone(),
                },
            ),
            Rc::new(
                super::datastores::LinksStore {},
            ),
            Rc::new(
                khronos_runtime::traits::ir::datastores::CopyDataStore {}
            ),
            Rc::new(
                super::datastores::TriggerStore {
                    guild_state: guild_state.clone(),
                    manager,
                }
            )
        ]
    }

    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(
        guild_state: Rc<GuildState>, 
        template_data: Arc<Template>, 
        runtime_shareable_data: khronos_runtime::rt::RuntimeShareableData,
        manager: khronos_runtime::rt::KhronosRuntimeManager<TemplateAssetManager>
    ) -> Self {
        Self {
            datastores: Self::datastores(guild_state.clone(), template_data.clone(), manager),
            guild_state,
            template_data,
            runtime_shareable_data,
        }
    }
}

impl KhronosContext for TemplateContextProvider {
    type Data = Arc<Template>;
    type KVProvider = ArKVProvider;
    type DiscordProvider = ArDiscordProvider;
    type LockdownDataStore = LockdownData;
    type LockdownProvider = ArLockdownProvider;
    type UserInfoProvider = ArUserInfoProvider;
    type PageProvider = ArPageProvider;
    type ScheduledExecProvider = ArScheduledExecProvider;
    type DataStoreProvider = ArDataStoreProvider;

    fn data(&self) -> Self::Data {
        self.template_data.clone()
    }

    fn allowed_caps(&self) -> &[String] {
        self.template_data.allowed_caps.as_ref()
    }

    /// Returns if the current context has a specific capability
    fn has_cap(&self, cap: &str) -> bool {
        for allowed_cap in self.template_data.allowed_caps.iter() {
            if allowed_cap == cap || (allowed_cap == "*" && cap != "assetmanager:use_bundled_templating_types") {
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

    fn kv_provider(&self, scope: ExecutorScope, kv_scope: &str) -> Option<Self::KVProvider> {
        Some(ArKVProvider {
            guild_id: match scope {
                ExecutorScope::ThisGuild => self.guild_state.guild_id,
                ExecutorScope::OwnerGuild => self
                    .template_data
                    .shop_owner
                    .unwrap_or(self.guild_state.guild_id),
            },
            guild_state: self.guild_state.clone(),
            kv_scope: kv_scope.to_string()
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

    fn scheduled_exec_provider(&self) -> Option<Self::ScheduledExecProvider> {
        Some(ArScheduledExecProvider {
            guild_state: self.guild_state.clone(),
        })
    }

    fn datastore_provider(&self, scope: ExecutorScope) -> Option<Self::DataStoreProvider> {
        Some(ArDataStoreProvider {
            guild_id: match scope {
                ExecutorScope::ThisGuild => self.guild_state.guild_id,
                ExecutorScope::OwnerGuild => self
                    .template_data
                    .shop_owner
                    .unwrap_or(self.guild_state.guild_id),
            },
            guild_state: self.guild_state.clone(),
            datastores: self.datastores.clone(),
        })
    }
}

#[derive(Clone)]
pub struct ArKVProvider {
    guild_id: serenity::all::GuildId,
    guild_state: Rc<GuildState>,
    kv_scope: String,
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
            "SELECT value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scope = $3",
        )
        .bind(self.guild_id.to_string())
        .bind(&key)
        .bind(&self.kv_scope)
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

    async fn list_scopes(&self) -> Result<Vec<String>, silverpelt::Error> {
        let rec = sqlx::query("SELECT scope FROM guild_templates_kv WHERE guild_id = $1 GROUP BY scope")
            .bind(self.guild_id.to_string())
            .fetch_all(&self.guild_state.pool)
            .await?;

        let mut scopes = vec![];

        for rec in rec {
            scopes.push(rec.try_get("scope")?);
        }

        Ok(scopes)
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

        let rec: i64 = sqlx::query("SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1 AND scope = $2")
            .bind(self.guild_id.to_string())
            .bind(&self.kv_scope)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
            .try_get::<Option<i64>, _>("count")?
            .unwrap_or_default();

        if rec
            >= TryInto::<i64>::try_into(
                self.guild_state.kv_constraints.max_keys,
            )?
        {
            return Err("Max keys limit reached".into());
        }

        sqlx::query(
            "INSERT INTO guild_templates_kv (guild_id, key, value, scope) VALUES ($1, $2, $3, $4) ON CONFLICT (guild_id, key, scope) DO UPDATE SET value = $3, last_updated_at = NOW()",
        )
        .bind(self.guild_id.to_string())
        .bind(key)
        .bind(data)
        .bind(&self.kv_scope)
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

        sqlx::query("DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scope = $3")
            .bind(self.guild_id.to_string())
            .bind(key)
            .bind(&self.kv_scope)
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
            "SELECT key, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND scope = $2 AND key ILIKE $3",
        )
        .bind(self.guild_id.to_string())
        .bind(&self.kv_scope)
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
            sqlx::query("SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1 AND key = $2 AND scope = $3")
                .bind(self.guild_id.to_string())
                .bind(key)
                .bind(&self.kv_scope)
                .fetch_one(&self.guild_state.pool)
                .await?
                .try_get::<Option<i64>, _>("count")?
                .unwrap_or_default();

        Ok(rec > 0)
    }

    async fn keys(&self) -> Result<Vec<String>, silverpelt::Error> {
        let rec = sqlx::query("SELECT key FROM guild_templates_kv WHERE guild_id = $1 AND scope = $2")
            .bind(self.guild_id.to_string())
            .bind(&self.kv_scope)
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
pub struct ArScheduledExecProvider {
    guild_state: Rc<GuildState>,
}

impl ScheduledExecProvider for ArScheduledExecProvider {
    fn attempt_action(&self, bucket: &str) -> Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.scheduled_execs.check(bucket)
    }

    /// Lists all scheduled executions
    async fn list(
        &self,
        id: Option<String>,
    ) -> Result<Vec<ScheduledExecution>, silverpelt::Error> {
        #[derive(sqlx::FromRow)]
        struct ScheduledExecutionRow {
            exec_id: String,
            template_name: String,
            data: serde_json::Value,
            run_at: chrono::DateTime<chrono::Utc>,
        }

        let scheduled_execs: Vec<ScheduledExecutionRow> = if let Some(id) = id {
            sqlx::query_as(
                "SELECT exec_id, template_name, data, run_at FROM scheduled_executions WHERE guild_id = $1 AND exec_id = $2",
            )
            .bind(self.guild_state.guild_id.to_string())
            .bind(id)
            .fetch_all(&self.guild_state.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT exec_id, template_name, data, run_at FROM scheduled_executions WHERE guild_id = $1",
            )
            .bind(self.guild_state.guild_id.to_string())
            .fetch_all(&self.guild_state.pool)
            .await?
        };

        let mut executions = vec![];

        for rec in scheduled_execs {
            let execution = ScheduledExecution {
                id: rec.exec_id,
                template_name: rec.template_name,
                data: rec.data,
                run_at: rec.run_at,
            };

            executions.push(execution);
        }

        Ok(executions)
    } 

    /// Adds a new scheduled execution
    async fn add(
        &self,
        exec: ScheduledExecution
    ) -> Result<(), silverpelt::Error> {
        sqlx::query(
            "INSERT INTO scheduled_executions (guild_id, exec_id, template_name, data, run_at) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(self.guild_state.guild_id.to_string())
        .bind(exec.id)
        .bind(exec.template_name)
        .bind(exec.data)
        .bind(exec.run_at)
        .execute(&self.guild_state.pool)
        .await?;

        // Regenerate the cache
        crate::templatingrt::cache::get_all_guild_scheduled_executions_from_db(
            self.guild_state.guild_id, 
            &self.guild_state.pool
        ).await?;

        Ok(())
    }

    /// Removes a scheduled execution
    async fn remove(
        &self,
        exec_id: String
    ) -> Result<(), silverpelt::Error> {
        crate::templatingrt::cache::remove_scheduled_execution(
            self.guild_state.guild_id, 
            &exec_id,
            &self.guild_state.pool, 
        ).await?;

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
        self.guild_state.ratelimits.data_stores.check(&format!("{}:{}", method, bucket))
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