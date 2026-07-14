use crate::worker::workerstate::WorkerState;
use crate::worker::workervmmanager::Id;
use dapi::{ChannelId, GuildId, UserId, controller::{DiscordProvider, DiscordProviderContext}, dhttp::Client};
use serde_json::Value;

#[derive(Clone)]
pub(super) struct ArDiscordProvider {
    pub id: Id,
    pub state: WorkerState,
}

impl ArDiscordProvider {
    const DISCLAIMER: &str = "Content provided by users is the sole responsibility of the author. AntiRaid does not monitor, verify, or endorse any user-generated messages.";

    fn guild_id(&self) -> Result<GuildId, crate::Error> {
        match self.id {
            Id::Guild(guild_id) => Ok(guild_id),
            Id::User(_) => Err("Current context is not a guild".into()),
        }
    }
}

impl DiscordProvider for ArDiscordProvider {
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
    ) -> Result<Value, crate::Error> {
        let guild_id = self.guild_id()?;
        let obj = self.state.stratum.guild(guild_id).await?;
        let Some(obj) = obj else { return Ok(serde_json::Value::Null) };
        Ok(obj)
    }

    async fn get_guild_member(
        &self,
        user_id: UserId,
    ) -> Result<Value, crate::Error> {
        let guild_id = self.guild_id()?;
        let obj = self.state.stratum.guild_member(guild_id, user_id).await?;
        let Some(obj) = obj else { return Ok(serde_json::Value::Null) };
        Ok(obj)
    }

    async fn get_guild_channels(
        &self,
    ) -> Result<Value, crate::Error> {
        let guild_id = self.guild_id()?;
        let obj = self.state.stratum.guild_channels(guild_id).await?;
        let Some(obj) = obj else { return Ok(serde_json::Value::Null) };
        Ok(obj)
    }

    async fn get_guild_roles(
        &self,
    ) -> Result<Value, crate::Error>
    {
        let guild_id = self.guild_id()?;
        let obj = self.state.stratum.guild_roles(guild_id).await?;
        let Some(obj) = obj else { return Ok(serde_json::Value::Null) };
        Ok(obj)
    }

    async fn get_channel(
        &self,
        channel_id: ChannelId,
    ) -> Result<Value, crate::Error> {
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

    fn dhttp(&self) -> &Client {
        &self.state.stratum.discord_http()
    }
}
