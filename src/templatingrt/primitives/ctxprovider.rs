use crate::templatingrt::state::GuildState;
use crate::templatingrt::template::Template;
use antiraid_types::stings::{Sting, StingAggregate};
use antiraid_types::userinfo::UserInfo;
use khronos_runtime::traits::context::KhronosContext;
use khronos_runtime::traits::discordprovider::DiscordProvider;
use khronos_runtime::traits::ir::{kv::KvRecord, Lockdown};
use khronos_runtime::traits::kvprovider::KVProvider;
use khronos_runtime::traits::lockdownprovider::LockdownProvider;
use khronos_runtime::traits::pageprovider::PageProvider;
use khronos_runtime::traits::stingprovider::StingProvider;
use khronos_runtime::traits::userinfoprovider::UserInfoProvider;
use khronos_runtime::utils::executorscope::ExecutorScope;
use moka::future::Cache;
use serenity::all::InteractionId;
use silverpelt::stings::{StingAggregateOperations, StingCreateOperations, StingOperations};
use silverpelt::userinfo::{NoMember, UserInfoOperations};
use std::num::TryFromIntError;
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
    pub guild_state: Rc<GuildState>,

    /// The template data
    pub template_data: Arc<Template>,

    /// The global table used
    pub global_table: mlua::Table,
}

impl KhronosContext for TemplateContextProvider {
    type Data = Arc<Template>;
    type KVProvider = ArKVProvider;
    type DiscordProvider = ArDiscordProvider;
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

    fn lockdown_provider(&self, scope: ExecutorScope) -> Option<Self::LockdownProvider> {
        Some(ArLockdownProvider {
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

    fn global_table(&self) -> mlua::Table {
        self.global_table.clone()
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

        let rec = sqlx::query!(
            "SELECT value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
            self.guild_id.to_string(),
            key
        )
        .fetch_optional(&self.guild_state.pool)
        .await?;

        let Some(rec) = rec else {
            return Ok(None);
        };

        Ok(Some(KvRecord {
            key,
            value: rec.value.unwrap_or(serde_json::Value::Null),
            created_at: Some(rec.created_at),
            last_updated_at: Some(rec.last_updated_at),
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

        let rec = sqlx::query!(
            "SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1",
            self.guild_id.to_string(),
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;

        if rec.count.unwrap_or(0)
            >= self
                .guild_state
                .kv_constraints
                .max_keys
                .try_into()
                .map_err(|e: TryFromIntError| e.to_string())?
        {
            return Err("Max keys limit reached".into());
        }

        sqlx::query!(
            "INSERT INTO guild_templates_kv (guild_id, key, value) VALUES ($1, $2, $3) ON CONFLICT (guild_id, key) DO UPDATE SET value = $3, last_updated_at = NOW()",
            self.guild_id.to_string(),
            key,
            data,
        )
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

        sqlx::query!(
            "DELETE FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
            self.guild_id.to_string(),
            key,
        )
        .execute(&self.guild_state.pool)
        .await?;

        Ok(())
    }

    async fn find(&self, query: String) -> Result<Vec<KvRecord>, silverpelt::Error> {
        // Check key length
        if query.len() > self.guild_state.kv_constraints.max_key_length {
            return Err("Query length too long".into());
        }

        let rec = sqlx::query!(
            "SELECT key, value, created_at, last_updated_at FROM guild_templates_kv WHERE guild_id = $1 AND key ILIKE $2",
            self.guild_id.to_string(),
            query
        )
        .fetch_all(&self.guild_state.pool)
        .await?;

        let mut records = vec![];

        for rec in rec {
            let record = KvRecord {
                key: rec.key,
                value: rec.value.unwrap_or(serde_json::Value::Null),
                created_at: Some(rec.created_at),
                last_updated_at: Some(rec.last_updated_at),
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

        let rec = sqlx::query!(
            "SELECT COUNT(*) FROM guild_templates_kv WHERE guild_id = $1 AND key = $2",
            self.guild_id.to_string(),
            key
        )
        .fetch_one(&self.guild_state.pool)
        .await?;

        Ok(rec.count.unwrap_or(0) > 0)
    }

    async fn keys(&self) -> Result<Vec<String>, silverpelt::Error> {
        let rec = sqlx::query!(
            "SELECT key FROM guild_templates_kv WHERE guild_id = $1",
            self.guild_id.to_string(),
        )
        .fetch_all(&self.guild_state.pool)
        .await?;

        let mut keys = vec![];

        for rec in rec {
            keys.push(rec.key);
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

    async fn get_audit_logs(
        &self,
        action_type: Option<serenity::all::audit_log::Action>,
        user_id: Option<serenity::model::prelude::UserId>,
        before: Option<serenity::model::prelude::AuditLogEntryId>,
        limit: Option<serenity::nonmax::NonMaxU8>,
    ) -> Result<serenity::model::prelude::AuditLogs, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .get_audit_logs(self.guild_id, action_type, user_id, before, limit)
            .await
            .map_err(|e| format!("Failed to fetch audit logs: {}", e).into())
    }

    async fn get_automod_rules(
        &self,
    ) -> Result<Vec<serenity::model::guild::automod::Rule>, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .get_automod_rules(self.guild_id)
            .await
            .map_err(|e| format!("Failed to fetch automod rules: {}", e).into())
    }

    async fn get_automod_rule(
        &self,
        rule_id: serenity::all::RuleId,
    ) -> Result<serenity::model::guild::automod::Rule, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .get_automod_rule(self.guild_id, rule_id)
            .await
            .map_err(|e| format!("Failed to fetch automod rule: {}", e).into())
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

    async fn create_member_ban(
        &self,
        user_id: serenity::all::UserId,
        delete_message_seconds: u32,
        reason: Option<&str>,
    ) -> Result<(), silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .ban_user(
                self.guild_id,
                user_id,
                (delete_message_seconds / 86400)
                    .try_into()
                    .map_err(|e| format!("Failed to convert ban duration to days: {}", e))?,
                reason,
            )
            .await
            .map_err(|e| format!("Failed to ban user: {}", e).into())
    }

    async fn kick_member(
        &self,
        user_id: serenity::all::UserId,
        reason: Option<&str>,
    ) -> Result<(), silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .kick_member(self.guild_id, user_id, reason)
            .await
            .map_err(|e| format!("Failed to kick user: {}", e).into())
    }

    async fn edit_member(
        &self,
        user_id: serenity::all::UserId,
        map: impl serde::Serialize,
        audit_log_reason: Option<&str>,
    ) -> Result<serenity::all::Member, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .edit_member(self.guild_id, user_id, &map, audit_log_reason)
            .await
            .map_err(|e| format!("Failed to edit member: {}", e).into())
    }

    async fn create_message(
        &self,
        channel_id: serenity::all::ChannelId,
        files: Vec<serenity::all::CreateAttachment<'_>>,
        data: impl serde::Serialize,
    ) -> Result<serenity::model::channel::Message, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .send_message(channel_id, files, &data)
            .await
            .map_err(|e| format!("Failed to send message: {}", e).into())
    }

    async fn create_interaction_response(
        &self,
        interaction_id: InteractionId,
        interaction_token: &str,
        response: impl serde::Serialize,
        files: Vec<serenity::all::CreateAttachment<'_>>,
    ) -> Result<(), silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .create_interaction_response(interaction_id, interaction_token, &response, files)
            .await
            .map_err(|e| format!("Failed to create interaction response: {}", e).into())
    }

    async fn create_followup_message(
        &self,
        interaction_token: &str,
        response: impl serde::Serialize,
        files: Vec<serenity::all::CreateAttachment<'_>>,
    ) -> Result<serenity::all::Message, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .create_followup_message(interaction_token, &response, files)
            .await
            .map_err(|e| format!("Failed to create interaction followup: {}", e).into())
    }

    async fn get_original_interaction_response(
        &self,
        interaction_token: &str,
    ) -> Result<serenity::model::channel::Message, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .get_original_interaction_response(interaction_token)
            .await
            .map_err(|e| format!("Failed to get original interaction response: {}", e).into())
    }

    async fn get_guild_commands(&self) -> Result<Vec<serenity::all::Command>, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .get_guild_commands(self.guild_id)
            .await
            .map_err(|e| format!("Failed to get guild commands: {}", e).into())
    }

    async fn get_guild_command(
        &self,
        command_id: serenity::all::CommandId,
    ) -> Result<serenity::all::Command, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .get_guild_command(self.guild_id, command_id)
            .await
            .map_err(|e| format!("Failed to get guild command: {}", e).into())
    }

    async fn create_guild_command(
        &self,
        map: impl serde::Serialize,
    ) -> Result<serenity::all::Command, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .create_guild_command(self.guild_id, &map)
            .await
            .map_err(|e| format!("Failed to create guild command: {}", e).into())
    }

    async fn get_messages(
        &self,
        channel_id: serenity::all::ChannelId,
        target: Option<serenity::all::MessagePagination>,
        limit: Option<serenity::nonmax::NonMaxU8>,
    ) -> Result<Vec<serenity::all::Message>, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .get_messages(channel_id, target, limit)
            .await
            .map_err(|e| format!("Failed to get messages: {}", e).into())
    }

    async fn get_message(
        &self,
        channel_id: serenity::all::ChannelId,
        message_id: serenity::all::MessageId,
    ) -> Result<serenity::all::Message, silverpelt::Error> {
        self.guild_state
            .serenity_context
            .http
            .get_message(channel_id, message_id)
            .await
            .map_err(|e| format!("Failed to get message: {}", e).into())
    }
}

#[derive(Clone)]
pub struct ArLockdownProvider {
    guild_id: serenity::all::GuildId,
    guild_state: Rc<GuildState>,
}

impl LockdownProvider for ArLockdownProvider {
    fn attempt_action(&self, bucket: &str) -> serenity::Result<(), silverpelt::Error> {
        self.guild_state.ratelimits.lockdowns.check(bucket)
    }

    async fn list(&self) -> Result<Vec<Lockdown>, silverpelt::Error> {
        let lockdowns = lockdowns::LockdownSet::guild(self.guild_id, &self.guild_state.pool)
            .await
            .map_err(|e| format!("Error while fetching lockdown set: {}", e))?;

        let mut result = vec![];

        for lockdown in lockdowns.lockdowns {
            result.push(Lockdown {
                id: lockdown.id,
                reason: lockdown.reason,
                r#type: lockdown.r#type.string_form(),
                data: lockdown.data,
                created_at: lockdown.created_at,
            });
        }

        Ok(result)
    }

    async fn qsl(&self, reason: String) -> Result<sqlx::types::uuid::Uuid, silverpelt::Error> {
        let mut lockdowns = lockdowns::LockdownSet::guild(self.guild_id, &self.guild_state.pool)
            .await
            .map_err(|e| format!("Error while fetching lockdown set: {}", e))?;

        // Create the lockdown
        let lockdown_type = lockdowns::qsl::QuickServerLockdown {};

        let lockdown_data = lockdowns::LockdownData {
            cache: &self.guild_state.serenity_context.cache,
            http: &self.guild_state.serenity_context.http,
            pool: self.guild_state.pool.clone(),
            reqwest: self.guild_state.reqwest_client.clone(),
        };

        let id = lockdowns
            .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
            .await
            .map_err(|e| format!("Error while applying lockdown: {}", e))?;

        Ok(id)
    }

    async fn tsl(&self, reason: String) -> Result<sqlx::types::uuid::Uuid, silverpelt::Error> {
        let mut lockdowns = lockdowns::LockdownSet::guild(self.guild_id, &self.guild_state.pool)
            .await
            .map_err(|e| format!("Error while fetching lockdown set: {}", e))?;

        // Create the lockdown
        let lockdown_type = lockdowns::tsl::TraditionalServerLockdown {};

        let lockdown_data = lockdowns::LockdownData {
            cache: &self.guild_state.serenity_context.cache,
            http: &self.guild_state.serenity_context.http,
            pool: self.guild_state.pool.clone(),
            reqwest: self.guild_state.reqwest_client.clone(),
        };

        let id = lockdowns
            .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
            .await
            .map_err(|e| format!("Error while applying lockdown: {}", e))?;

        Ok(id)
    }

    async fn scl(
        &self,
        channel_id: serenity::all::ChannelId,
        reason: String,
    ) -> Result<sqlx::types::uuid::Uuid, silverpelt::Error> {
        let mut lockdowns = lockdowns::LockdownSet::guild(self.guild_id, &self.guild_state.pool)
            .await
            .map_err(|e| format!("Error while fetching lockdown set: {}", e))?;

        // Create the lockdown
        let lockdown_type = lockdowns::scl::SingleChannelLockdown(channel_id);

        let lockdown_data = lockdowns::LockdownData {
            cache: &self.guild_state.serenity_context.cache,
            http: &self.guild_state.serenity_context.http,
            pool: self.guild_state.pool.clone(),
            reqwest: self.guild_state.reqwest_client.clone(),
        };

        let id = lockdowns
            .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
            .await
            .map_err(|e| format!("Error while applying lockdown: {}", e))?;

        Ok(id)
    }

    async fn role(
        &self,
        role_id: serenity::all::RoleId,
        reason: String,
    ) -> Result<sqlx::types::uuid::Uuid, silverpelt::Error> {
        let mut lockdowns = lockdowns::LockdownSet::guild(self.guild_id, &self.guild_state.pool)
            .await
            .map_err(|e| format!("Error while fetching lockdown set: {}", e))?;

        // Create the lockdown
        let lockdown_type = lockdowns::role::RoleLockdown(role_id);

        let lockdown_data = lockdowns::LockdownData {
            cache: &self.guild_state.serenity_context.cache,
            http: &self.guild_state.serenity_context.http,
            pool: self.guild_state.pool.clone(),
            reqwest: self.guild_state.reqwest_client.clone(),
        };

        let id = lockdowns
            .easy_apply(Box::new(lockdown_type), &lockdown_data, &reason)
            .await
            .map_err(|e| format!("Error while applying lockdown: {}", e))?;

        Ok(id)
    }

    async fn remove(&self, id: sqlx::types::uuid::Uuid) -> Result<(), silverpelt::Error> {
        let mut lockdowns = lockdowns::LockdownSet::guild(self.guild_id, &self.guild_state.pool)
            .await
            .map_err(|e| format!("Error while fetching lockdown set: {}", e))?;

        let lockdown_data = lockdowns::LockdownData {
            cache: &self.guild_state.serenity_context.cache,
            http: &self.guild_state.serenity_context.http,
            pool: self.guild_state.pool.clone(),
            reqwest: self.guild_state.reqwest_client.clone(),
        };

        lockdowns
            .easy_remove(id, &lockdown_data)
            .await
            .map_err(|e| format!("Error while applying lockdown: {}", e))?;

        Ok(())
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
            .create_and_dispatch_returning_id(
                self.guild_state.serenity_context.clone(),
                &self.guild_state.pool,
            )
            .await?;

        Ok(sting)
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
            .update_and_dispatch(
                &self.guild_state.pool,
                self.guild_state.serenity_context.clone(),
            )
            .await
            .map_err(|e| format!("failed to update sting: {}", e))?;

        todo!()
    }

    async fn delete(&self, id: sqlx::types::uuid::Uuid) -> Result<(), silverpelt::Error> {
        let Some(sting) = Sting::get(&self.guild_state.pool, self.guild_id, id)
            .await
            .map_err(|e| format!("failed to fetch sting information: {:?}", e))?
        else {
            return Err("Sting not found".into());
        };

        if sting.guild_id != self.guild_id {
            return Err("sting not associated with this guild".into());
        }

        sting
            .delete_and_dispatch(
                &self.guild_state.pool,
                self.guild_state.serenity_context.clone(),
            )
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
