use std::borrow::Cow;
use std::collections::HashMap;

use arrayvec::ArrayVec;
use nonmax::NonMaxU16;
use serde::de::Error;
use serde::{ser::SerializeSeq, Deserialize, Serialize};
use serde_json::Value;
use serenity::all::*;

use crate::lang_lua::plugins::antiraid::typesext::MultiOption;

#[derive(Deserialize, Debug, Default, Clone)]
pub struct SingleCreateMessageAttachment<'a> {
    pub filename: Cow<'static, str>,
    pub description: Option<Cow<'a, str>>,
    pub content: Vec<u8>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExistingAttachment {
    id: AttachmentId,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)] // Serde needs to do either id only for existing or filename/description/content for new
pub enum NewOrExisting<'a> {
    New(SingleCreateMessageAttachment<'a>),
    Existing(ExistingAttachment),
}

#[derive(Deserialize, Debug, Default, Clone)]
pub struct CreateMessageAttachment<'a> {
    #[serde(flatten)]
    pub new_and_existing_attachments: Vec<NewOrExisting<'a>>,
}

impl Serialize for CreateMessageAttachment<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct NewAttachment<'a> {
            id: u64,
            filename: &'a Cow<'static, str>,
            description: &'a Option<Cow<'a, str>>,
        }

        // Instead of an `AttachmentId`, the `id` field for new attachments corresponds to the
        // index of the new attachment in the multipart payload. The attachment data will be
        // labeled with `files[{id}]` in the multipart body. See `Multipart::build_form`.
        let mut id = 0;
        let mut seq = serializer.serialize_seq(Some(self.new_and_existing_attachments.len()))?;
        for attachment in &self.new_and_existing_attachments {
            match attachment {
                NewOrExisting::New(new_attachment) => {
                    let attachment = NewAttachment {
                        id,
                        filename: &new_attachment.filename,
                        description: &new_attachment.description,
                    };
                    id += 1;
                    seq.serialize_element(&attachment)?;
                }
                NewOrExisting::Existing(existing_attachment) => {
                    seq.serialize_element(existing_attachment)?;
                }
            }
        }
        seq.end()
    }
}

impl<'a> CreateMessageAttachment<'a> {
    pub fn take_files(&self) -> Result<Vec<serenity::all::CreateAttachment<'a>>, crate::Error> {
        pub const MESSAGE_ATTACHMENT_DESCRIPTION_LIMIT: usize = 1024;
        pub const MESSAGE_ATTACHMENT_CONTENT_BYTES_LIMIT: usize = 8 * 1024 * 1024; // 8 MB
        pub const MESSAGE_MAX_ATTACHMENT_COUNT: usize = 3;

        if self.new_and_existing_attachments.len() > MESSAGE_MAX_ATTACHMENT_COUNT {
            return Err(format!(
                "Too many attachments, limit is {}",
                MESSAGE_MAX_ATTACHMENT_COUNT
            )
            .into());
        }

        let mut attachments = Vec::new();
        for attachment in &self.new_and_existing_attachments {
            if let NewOrExisting::New(new_attachment) = attachment {
                let desc = new_attachment
                    .description
                    .as_ref()
                    .unwrap_or(&Cow::Borrowed(""));

                if desc.len() > MESSAGE_ATTACHMENT_DESCRIPTION_LIMIT {
                    return Err(format!(
                        "Attachment description exceeds limit of {}",
                        MESSAGE_ATTACHMENT_DESCRIPTION_LIMIT
                    )
                    .into());
                }

                let content = &new_attachment.content;

                if content.is_empty() {
                    return Err("Attachment content cannot be empty".into());
                }

                if content.len() > MESSAGE_ATTACHMENT_CONTENT_BYTES_LIMIT {
                    return Err(format!(
                        "Attachment content exceeds limit of {} bytes",
                        MESSAGE_ATTACHMENT_CONTENT_BYTES_LIMIT
                    )
                    .into());
                }

                let mut ca = serenity::all::CreateAttachment::bytes(
                    content.clone(),
                    new_attachment.filename.clone(),
                );

                if !desc.is_empty() {
                    ca = ca.description(desc.clone());
                }

                attachments.push(ca);
            }
        }

        Ok(attachments)
    }
}

/// [Discord docs](https://discord.com/developers/docs/resources/channel#create-message)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[must_use]
pub struct CreateMessage<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nonce: Option<Nonce>,
    #[serde(default)]
    pub tts: bool,
    #[serde(default)]
    pub embeds: Cow<'a, [CreateEmbed<'a>]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<CreateAllowedMentions<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_reference: Option<MessageReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Cow<'a, [ActionRow]>>,
    #[serde(default)]
    pub sticker_ids: Cow<'a, [StickerId]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<MessageFlags>,
    #[serde(default)]
    pub enforce_nonce: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll: Option<CreatePoll<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<CreateMessageAttachment<'a>>,
}

/// [Discord docs](https://discord.com/developers/docs/interactions/receiving-and-responding#interaction-response-object).
#[derive(Clone, Debug)]
pub enum CreateInteractionResponse<'a> {
    /// Acknowledges a Ping (only required when your bot uses an HTTP endpoint URL).
    ///
    /// Corresponds to Discord's `PONG`.
    Pong,
    /// Responds to an interaction with a message.
    ///
    /// Corresponds to Discord's `CHANNEL_MESSAGE_WITH_SOURCE`.
    Message(CreateInteractionResponseMessage<'a>),
    /// Acknowledges the interaction in order to edit a response later. The user sees a loading
    /// state.
    ///
    /// Corresponds to Discord's `DEFERRED_CHANNEL_MESSAGE_WITH_SOURCE`.
    Defer(CreateInteractionResponseMessage<'a>),
    /// Only valid for component-based interactions (seems to work for modal submit interactions
    /// too even though it's not documented).
    ///
    /// Acknowledges the interaction. You can optionally edit the original message later. The user
    /// does not see a loading state.
    ///
    /// Corresponds to Discord's `DEFERRED_UPDATE_MESSAGE`.
    Acknowledge,
    /// Only valid for component-based interactions.
    ///
    /// Edits the message the component was attached to.
    ///
    /// Corresponds to Discord's `UPDATE_MESSAGE`.
    UpdateMessage(CreateInteractionResponseMessage<'a>),
    /// Only valid for autocomplete interactions.
    ///
    /// Responds to the autocomplete interaction with suggested choices.
    ///
    /// Corresponds to Discord's `APPLICATION_COMMAND_AUTOCOMPLETE_RESULT`.
    Autocomplete(CreateAutocompleteResponse<'a>),
    /// Not valid for Modal and Ping interactions
    ///
    /// Responds to the interaction with a popup modal.
    ///
    /// Corresponds to Discord's `MODAL`.
    Modal(CreateModal<'a>),
    /// Not valid for autocomplete and Ping interactions. Only available for applications with
    /// Activities enabled.
    ///
    /// Responds to the interaction by launching the Activity associated with the app.
    ///
    /// Corresponds to Discord's `LAUNCH_ACTIVITY`.
    LaunchActivity,
}

impl Default for CreateInteractionResponse<'_> {
    fn default() -> Self {
        Self::Message(Default::default())
    }
}

impl<'a> CreateInteractionResponse<'a> {
    pub fn take_files(&self) -> Result<Vec<serenity::all::CreateAttachment<'a>>, crate::Error> {
        match self {
            Self::Message(x) => {
                if let Some(ref x) = x.attachments {
                    x.take_files()
                } else {
                    Ok(Vec::new())
                }
            }
            Self::Defer(x) => {
                if let Some(ref x) = x.attachments {
                    x.take_files()
                } else {
                    Ok(Vec::new())
                }
            }
            Self::UpdateMessage(x) => {
                if let Some(ref x) = x.attachments {
                    x.take_files()
                } else {
                    Ok(Vec::new())
                }
            }
            _ => Ok(Vec::new()),
        }
    }
}

/// [Discord docs](https://discord.com/developers/docs/interactions/receiving-and-responding#interaction-response-object-messages).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[must_use]
pub struct CreateInteractionResponseMessage<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tts: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embeds: Option<Cow<'a, [CreateEmbed<'a>]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_mentions: Option<CreateAllowedMentions<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<InteractionResponseFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub components: Option<Cow<'a, [ActionRow]>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub poll: Option<CreatePoll<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachments: Option<CreateMessageAttachment<'a>>,
}

impl serde::Serialize for CreateInteractionResponse<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap as _;

        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry(
            "type",
            &match self {
                Self::Pong => 1,
                Self::Message(_) => 4,
                Self::Defer(_) => 5,
                Self::Acknowledge => 6,
                Self::UpdateMessage(_) => 7,
                Self::Autocomplete(_) => 8,
                Self::Modal(_) => 9,
                Self::LaunchActivity => 12,
            },
        )?;

        match self {
            Self::Pong => map.serialize_entry("data", &None::<()>)?,
            Self::Message(x) => map.serialize_entry("data", &x)?,
            Self::Defer(x) => map.serialize_entry("data", &x)?,
            Self::Acknowledge => map.serialize_entry("data", &None::<()>)?,
            Self::UpdateMessage(x) => map.serialize_entry("data", &x)?,
            Self::Autocomplete(x) => map.serialize_entry("data", &x)?,
            Self::Modal(x) => map.serialize_entry("data", &x)?,
            Self::LaunchActivity => map.serialize_entry("data", &None::<()>)?,
        }

        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for CreateInteractionResponse<'_> {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let map = serde_json::Map::deserialize(deserializer)?;

        let raw_kind = map
            .get("type")
            .ok_or_else(|| D::Error::missing_field("type"))?
            .clone();
        let value = Value::from(map);

        let ty = raw_kind
            .as_u64()
            .ok_or_else(|| D::Error::custom("type must be a number"))?;

        match ty {
            1 => Ok(Self::Pong),
            4 => serde_json::from_value(value).map(Self::Message),
            5 => serde_json::from_value(value).map(Self::Defer),
            6 => Ok(Self::Acknowledge),
            7 => serde_json::from_value(value).map(Self::UpdateMessage),
            8 => serde_json::from_value(value).map(Self::Autocomplete),
            9 => serde_json::from_value(value).map(Self::Modal),
            12 => Ok(Self::LaunchActivity),
            _ => {
                return Err(D::Error::custom(format!(
                    "Unknown interaction response type: {}",
                    ty
                )));
            }
        }
        .map_err(D::Error::custom)
    }
}

/// [Discord docs](https://discord.com/developers/docs/interactions/receiving-and-responding#interaction-response-object-autocomplete)
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[must_use]
pub struct CreateAutocompleteResponse<'a> {
    choices: Cow<'a, [AutocompleteChoice<'a>]>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
#[non_exhaustive]
pub enum AutocompleteValue<'a> {
    String(Cow<'a, str>),
    Integer(u64),
    Float(f64),
}

// Same as CommandOptionChoice according to Discord, see
// [Autocomplete docs](https://discord.com/developers/docs/interactions/receiving-and-responding#interaction-response-object-autocomplete).
#[must_use]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AutocompleteChoice<'a> {
    pub name: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<HashMap<Cow<'a, str>, Cow<'a, str>>>,
    pub value: AutocompleteValue<'a>,
}

/// [Discord docs](https://discord.com/developers/docs/interactions/receiving-and-responding#interaction-response-object-modal).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[must_use]
pub struct CreateModal<'a> {
    components: Cow<'a, [ActionRow]>,
    custom_id: Cow<'a, str>,
    title: Cow<'a, str>,
}

/// A builder to create an embed in a message
///
/// [Discord docs](https://discord.com/developers/docs/resources/channel#embed-object)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use]
pub struct CreateEmbed<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<Cow<'a, str>>,
    #[serde(rename = "type")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<Timestamp>,
    #[serde(rename = "color")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<Colour>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footer: Option<CreateEmbedFooter<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<CreateEmbedImage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<CreateEmbedImage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<CreateEmbedAuthor<'a>>,
    /// No point using a Cow slice, as there is no set_fields method
    /// and CreateEmbedField is not public.
    #[serde(skip_serializing_if = "<[_]>::is_empty")]
    #[serde(default)]
    pub fields: Vec<CreateEmbedField<'a>>,
}

/// A builder to create the footer data for an embed.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use]
pub struct CreateEmbedFooter<'a> {
    pub text: Cow<'a, str>,
    pub icon_url: Option<Cow<'a, str>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateEmbedImage<'a> {
    pub url: Cow<'a, str>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateEmbedField<'a> {
    pub name: Cow<'a, str>,
    pub value: Cow<'a, str>,
    pub inline: bool,
}

/// A builder to create the author data of an embed. See [`CreateEmbed::author`]
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use]
pub struct CreateEmbedAuthor<'a> {
    pub name: Cow<'a, str>,
    pub url: Option<Cow<'a, str>>,
    pub icon_url: Option<Cow<'a, str>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ParseValue {
    Everyone,
    Users,
    Roles,
}

/// [Discord docs](https://discord.com/developers/docs/resources/channel#allowed-mentions-object).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[must_use]
pub struct CreateAllowedMentions<'a> {
    pub parse: ArrayVec<ParseValue, 3>,
    pub users: Cow<'a, [UserId]>,
    pub roles: Cow<'a, [RoleId]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replied_user: Option<bool>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreatePoll<'a> {
    pub question: CreatePollMedia<'a>,
    pub answers: Cow<'a, [CreatePollAnswer<'a>]>,
    pub duration: u8,
    pub allow_multiselect: bool,
    pub layout_type: Option<PollLayoutType>,
}

/// "Only text is supported."
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CreatePollMedia<'a> {
    pub text: Cow<'a, str>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CreatePollAnswerMedia<'a> {
    pub text: Option<Cow<'a, str>>,
    pub emoji: Option<PollMediaEmoji>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct CreatePollAnswer<'a> {
    pub poll_media: CreatePollAnswerMedia<'a>,
}

/// [Discord docs](https://discord.com/developers/docs/resources/channel#modify-channel-json-params-guild-channel).
///
/// Unlike Serenity, AntiRaid combines EditChannel and EditThread to allow using standard Discord typings
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use]
pub struct EditChannel<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    pub kind: Option<ChannelType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nsfw: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_per_user: Option<NonMaxU16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_limit: Option<NonMaxU16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub permission_overwrites: Option<Cow<'a, [PermissionOverwrite]>>,
    #[serde(skip_serializing_if = "MultiOption::should_not_serialize")]
    pub parent_id: MultiOption<ChannelId>,
    #[serde(skip_serializing_if = "MultiOption::should_not_serialize")]
    pub rtc_region: MultiOption<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_quality_mode: Option<VideoQualityMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flags: Option<ChannelFlags>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_tags: Option<Cow<'a, [CreateForumTag<'a>]>>,
    #[serde(skip_serializing_if = "MultiOption::should_not_serialize")]
    pub default_reaction_emoji: MultiOption<ForumEmoji>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_thread_rate_limit_per_user: Option<NonMaxU16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_sort_order: Option<SortOrder>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_forum_layout: Option<ForumLayoutType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    auto_archive_duration: Option<AutoArchiveDuration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    locked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    invitable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    applied_tags: Option<Cow<'a, [ForumTagId]>>,
}

impl Default for EditChannel<'_> {
    fn default() -> Self {
        Self {
            name: Some("my-channel".into()),
            kind: Some(serenity::all::ChannelType::Text),
            position: Some(7),
            topic: Some("My channel topic".into()),
            nsfw: Some(true),
            rate_limit_per_user: Some(serenity::nonmax::NonMaxU16::new(5).unwrap()),
            bitrate: None,
            permission_overwrites: None,
            parent_id: MultiOption::new(Some(serenity::all::ChannelId::default())),
            rtc_region: MultiOption::new(Some("us-west".into())),
            video_quality_mode: Some(serenity::all::VideoQualityMode::Auto),
            default_auto_archive_duration: Some(serenity::all::AutoArchiveDuration::OneDay),
            flags: Some(serenity::all::ChannelFlags::all()),
            available_tags: None,
            default_reaction_emoji: MultiOption::new(Some(serenity::all::ForumEmoji::Id(
                serenity::all::EmojiId::default(),
            ))),
            default_thread_rate_limit_per_user: None,
            default_sort_order: None,
            default_forum_layout: None,
            status: Some("online".into()),
            user_limit: Some(serenity::nonmax::NonMaxU16::new(10).unwrap()),
            archived: Some(false),
            auto_archive_duration: Some(serenity::all::AutoArchiveDuration::OneDay),
            locked: Some(false),
            invitable: Some(true),
            applied_tags: None,
        }
    }
}

/// [Discord docs](https://discord.com/developers/docs/resources/channel#forum-tag-object-forum-tag-structure)
///
/// Contrary to the [`ForumTag`] struct, only the name field is required.
#[must_use]
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateForumTag<'a> {
    pub name: Cow<'a, str>,
    pub moderated: bool,
    pub emoji_id: Option<EmojiId>,
    pub emoji_name: Option<Cow<'a, str>>,
}

/// A builder for creating a new [`Command`].
///
/// [`Command`]: crate::model::application::Command
///
/// Discord docs:
/// - [global command](https://discord.com/developers/docs/interactions/application-commands#create-global-application-command)
/// - [guild command](https://discord.com/developers/docs/interactions/application-commands#create-guild-application-command)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use]
pub struct CreateCommand<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "type")]
    kind: Option<CommandType>,
    #[serde(skip_serializing_if = "Option::is_none")]
    handler: Option<EntryPointHandlerType>,

    #[serde(flatten)]
    fields: EditCommand<'a>,
}

impl Default for CreateCommand<'_> {
    fn default() -> Self {
        Self {
            kind: Some(CommandType::ChatInput),
            handler: Some(EntryPointHandlerType::AppHandler),
            fields: Default::default(),
        }
    }
}

/// A builder for editing an existing [`Command`].
///
/// [`Command`]: crate::model::application::Command
///
/// Discord docs:
/// - [global command](https://discord.com/developers/docs/interactions/application-commands#edit-global-application-command)
/// - [guild command](https://discord.com/developers/docs/interactions/application-commands#edit-guild-application-command)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use]
pub struct EditCommand<'a> {
    name: Option<Cow<'a, str>>,
    name_localizations: HashMap<Cow<'a, str>, Cow<'a, str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<Cow<'a, str>>,
    description_localizations: HashMap<Cow<'a, str>, Cow<'a, str>>,
    options: Cow<'a, [CreateCommandOption<'a>]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_member_permissions: Option<Permissions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dm_permission: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    integration_types: Option<Vec<InstallationContext>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contexts: Option<Vec<InteractionContext>>,
    nsfw: bool,
}

impl Default for EditCommand<'_> {
    fn default() -> Self {
        Self {
            name: Some("my-command".into()),
            name_localizations: HashMap::new(),
            description: Some("My command description".into()),
            description_localizations: HashMap::new(),
            options: Cow::default(),
            default_member_permissions: None,
            dm_permission: None,
            integration_types: None,
            contexts: None,
            nsfw: false,
        }
    }
}

/// A builder for creating a new [`CommandOption`].
///
/// [`Self::kind`], [`Self::name`], and [`Self::description`] are required fields.
///
/// [`CommandOption`]: crate::model::application::CommandOption
///
/// [Discord docs](https://discord.com/developers/docs/interactions/application-commands#application-command-object-application-command-option-structure).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[must_use]
pub struct CreateCommandOption<'a> {
    #[serde(rename = "type")]
    kind: CommandOptionType,
    name: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name_localizations: Option<HashMap<Cow<'a, str>, Cow<'a, str>>>,
    description: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description_localizations: Option<HashMap<Cow<'a, str>, Cow<'a, str>>>,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    choices: Cow<'a, [CreateCommandOptionChoice<'a>]>,
    #[serde(default)]
    options: Cow<'a, [CreateCommandOption<'a>]>,
    #[serde(default)]
    channel_types: Cow<'a, [ChannelType]>,
    #[serde(default)]
    min_value: Option<serde_json::Number>,
    #[serde(default)]
    max_value: Option<serde_json::Number>,
    #[serde(default)]
    min_length: Option<u16>,
    #[serde(default)]
    max_length: Option<u16>,
    #[serde(default)]
    autocomplete: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CreateCommandOptionChoice<'a> {
    pub name: Cow<'a, str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name_localizations: Option<HashMap<Cow<'a, str>, Cow<'a, str>>>,
    pub value: Value,
}
