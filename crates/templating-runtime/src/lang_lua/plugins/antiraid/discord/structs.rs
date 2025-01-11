use super::builders::{CreateCommand, CreateInteractionResponse, CreateMessage, EditChannel};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GetAuditLogOptions {
    pub action_type: Option<serenity::all::audit_log::Action>,
    pub user_id: Option<serenity::all::UserId>,
    pub before: Option<serenity::all::AuditLogEntryId>,
    pub limit: Option<serenity::nonmax::NonMaxU8>,
}

impl Default for GetAuditLogOptions {
    fn default() -> Self {
        Self {
            action_type: Some(serenity::all::audit_log::Action::GuildUpdate),
            user_id: Some(serenity::all::UserId::default()),
            before: Some(serenity::all::AuditLogEntryId::default()),
            limit: Some(serenity::nonmax::NonMaxU8::default()),
        }
    }
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct GetChannelOptions {
    pub channel_id: serenity::all::ChannelId,
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct EditChannelOptions {
    pub channel_id: serenity::all::ChannelId,
    pub reason: String,
    pub data: EditChannel,
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct DeleteChannelOptions {
    pub channel_id: serenity::all::ChannelId,
    pub reason: String,
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct CreateMessageOptions {
    pub channel_id: serenity::all::ChannelId, // Channel *must* be in the same guild
    pub data: CreateMessage,
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct CreateCommandOptions {
    pub data: CreateCommand,
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct CreateInteractionResponseOptions {
    pub interaction_id: serenity::all::InteractionId,
    pub interaction_token: String,
    pub data: CreateInteractionResponse,
}
