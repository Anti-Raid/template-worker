use crate::worker::workerstate::WorkerState;
use crate::worker::workervmmanager::Id;
use dapi::controller::{DiscordProvider, DiscordProviderContext};
use serde_json::Value;
use std::sync::Arc;
use crate::worker::limits::Ratelimits;

#[derive(Clone)]
pub(super) struct ArDiscordProvider {
    pub id: Id,
    pub state: WorkerState,
    pub ratelimits: Arc<Ratelimits>,
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
