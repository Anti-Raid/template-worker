use super::builders::{CreateCommand, CreateMessage, EditChannel};

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
pub struct EditChannelOptions<'a> {
    pub channel_id: serenity::all::ChannelId,
    pub reason: &'a str,
    pub data: EditChannel<'a>,
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct DeleteChannelOptions<'a> {
    pub channel_id: serenity::all::ChannelId,
    pub reason: &'a str,
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct CreateMessageOptions<'a> {
    pub channel_id: serenity::all::ChannelId, // Channel *must* be in the same guild
    pub data: CreateMessage<'a>,
}

#[derive(serde::Serialize, Default, serde::Deserialize)]
pub struct CreateCommandOptions<'a> {
    pub command: CreateCommand<'a>,
}
