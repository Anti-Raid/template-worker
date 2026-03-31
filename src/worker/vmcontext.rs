use super::workerstate::WorkerState;
use super::workervmmanager::Id;
use crate::worker::builtins::EXPOSED_VFS;
use crate::worker::syscall::{SyscallArgs, SyscallHandler, SyscallRet};
use crate::worker::workertenantstate::WorkerTenantState;
use crate::worker::workervmmanager::VmData;
use khronos_runtime::core::typesext::Vfs;
use khronos_runtime::traits::context::KhronosContext;
use khronos_runtime::traits::ir::runtime as runtime_ir;
use dapi::controller::{DiscordProvider, DiscordProviderContext};
use khronos_runtime::traits::runtimeprovider::RuntimeProvider;
use serde_json::Value;
use std::sync::Arc;
use super::limits::Ratelimits;

#[derive(Clone)]
pub struct TemplateContextProvider {
    state: WorkerState,

    /// system call handler
    syscall_handler: SyscallHandler,

    id: Id,
    
    /// The ratelimits of the VM
    ratelimits: Arc<Ratelimits>,
}

impl TemplateContextProvider {
    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(
        id: Id,
        vm_data: VmData,
        wts: WorkerTenantState
    ) -> Self {
        Self {
            id,
            syscall_handler: SyscallHandler::new(vm_data.state.clone(), wts, vm_data.kv_constraints, vm_data.ratelimits.clone()),
            state: vm_data.state,
            ratelimits: vm_data.ratelimits,
        }
    }

    fn id(&self) -> Id {
        self.id.clone()
    }
}

impl KhronosContext for TemplateContextProvider {
    type DiscordProvider = ArDiscordProvider;
    type RuntimeProvider = ArRuntimeProvider;

    fn discord_provider(&self) -> Option<Self::DiscordProvider> {
        Some(ArDiscordProvider {
            id: self.id(),
            state: self.state.clone(),
            ratelimits: self.ratelimits.clone(),
        })
    }

    fn runtime_provider(&self) -> Option<Self::RuntimeProvider> {
        Some(ArRuntimeProvider {
            id: self.id(),
            state: self.state.clone(),
            syscall_handler: self.syscall_handler.clone(),
            ratelimits: self.ratelimits.clone(),
        })
    }
}

#[derive(Clone)]
pub struct ArDiscordProvider {
    id: Id,
    state: WorkerState,
    ratelimits: Arc<Ratelimits>,
}

impl ArDiscordProvider {
    const DISCLAIMER: &str = "Content provided by users is the sole responsibility of the author. AntiRaid does not monitor, verify, or endorse any user-generated messages.";

    fn guild_id(&self) -> Result<serenity::all::GuildId, crate::Error> {
        match self.id {
            Id::Guild(guild_id) => Ok(guild_id),
            Id::User(_) => Err("Current context is not a guild".into()),
        }
    }
}

impl DiscordProvider for ArDiscordProvider {
    fn attempt_action(&self, bucket: &str) -> serenity::Result<(), crate::Error> {
        self.ratelimits.discord.check(bucket)
    }

    // inject disclaimer into messages sent by the bot that are not interaction responses
    fn superuser_transform_message_before_send(&self, msg: dapi::controller::SuperUserMessageTransform, flags: dapi::controller::SuperUserMessageTransformFlags) -> Result<dapi::controller::SuperUserMessageTransform, dapi::Error> {
        if flags.is_interaction_response() {
            return Ok(msg) // Do not inject disclaimer in interaction responses (for now)
        }

        dapi::ensure_safe::inject_disclaimer(
            msg,
            Self::DISCLAIMER,
        )
    }

    async fn get_guild(
        &self,
    ) -> serenity::Result<Value, crate::Error> {
        let guild_id = self.guild_id()?;
        let obj = self.state.stratum.guild(guild_id).await?;
        let Some(obj) = obj else { return Ok(serde_json::Value::Null) };
        Ok(obj)
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
        let guild_id = self.guild_id()?;
        let obj = self.state.stratum.guild_member(guild_id, user_id).await?;
        let Some(obj) = obj else { return Ok(serde_json::Value::Null) };
        Ok(obj)
    }

    async fn get_guild_channels(
        &self,
    ) -> serenity::Result<Value, crate::Error> {
        let guild_id = self.guild_id()?;
        let obj = self.state.stratum.guild_channels(guild_id).await?;
        let Some(obj) = obj else { return Ok(serde_json::Value::Null) };
        Ok(obj)
    }

    async fn get_guild_roles(
        &self,
    ) -> serenity::Result<Value, crate::Error>
    {
        let guild_id = self.guild_id()?;
        let obj = self.state.stratum.guild_roles(guild_id).await?;
        let Some(obj) = obj else { return Ok(serde_json::Value::Null) };
        Ok(obj)
    }

    async fn get_channel(
        &self,
        channel_id: serenity::all::GenericChannelId,
    ) -> serenity::Result<Value, crate::Error> {
        let guild_id = self.guild_id()?;
        let channel = self.state.stratum.channel(channel_id).await?;

        let Some(channel) = channel else {
            return Ok(serde_json::Value::Null);
        };

        let Some(Value::String(channel_guild_id)) = channel.get("guild_id") else {
            return Err(format!("Channel {channel_id} does not belong to a guild").into());
        };

        if channel_guild_id != &guild_id.to_string() {
            return Err(format!("Channel {channel_id} does not belong to the guild").into());
        }

        Ok(channel)
    }

    fn context(&self) -> DiscordProviderContext {
        self.id.to_provider_context()
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
pub struct ArRuntimeProvider {
    id: Id,
    state: WorkerState,
    syscall_handler: SyscallHandler,
    ratelimits: Arc<Ratelimits>,
}

impl RuntimeProvider for ArRuntimeProvider {
    type SyscallArgs = SyscallArgs;
    type SyscallRet = SyscallRet;

    fn attempt_action(&self, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.ratelimits.runtime.check(bucket)
    }

    fn get_exposed_vfs(&self) -> Result<std::collections::HashMap<String, Vfs>, khronos_runtime::Error> {
        Ok((&*EXPOSED_VFS).clone())
    }

    async fn stats(&self) -> Result<runtime_ir::RuntimeStats, khronos_runtime::Error> {
        log::info!("Fetching runtime stats for tenant {:?}", self.id);
        let resp = self.state.stratum.get_status().await?;

        Ok(runtime_ir::RuntimeStats {
            total_cached_guilds: resp.guild_count, // This field is deprecated, use total_guilds instead
            total_guilds: resp.guild_count,
            total_users: resp.user_count,
            //total_members: sandwich_resp.total_members.try_into()?,
            last_started_at: crate::CONFIG.start_time,
        })
    }

    fn event_list(&self) -> Result<Vec<String>, khronos_runtime::Error> {
        let mut vec = dapi::EVENT_LIST
            .iter()
            .copied()
            .map(|x| x.to_string())
            .collect::<Vec<String>>();


        vec.push("OnStartup".to_string());
        vec.push("KeyExpiry".to_string());

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

    async fn syscall(&self, args: SyscallArgs) -> Result<SyscallRet, khronos_runtime::Error> {
        self.syscall_handler.handle_syscall(self.id, args).await
    }
}
