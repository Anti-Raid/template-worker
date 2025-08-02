use crate::dispatch::{parse_event, KhronosValueMapper};
use crate::templatingrt::cache::{DeferredCacheRegenMode, DEFERRED_CACHE_REGENS};
use crate::templatingrt::state::GuildState;
use crate::templatingrt::template::TemplateLanguage;
use crate::templatingrt::{KhronosValueResponse, MAX_TEMPLATES_RETURN_WAIT_TIME};
use antiraid_types::ar_event::{AntiraidEvent, KeyResumeEvent};
use chrono::Utc;
use indexmap::IndexMap;
use khronos_runtime::traits::ir::{DataStoreImpl, DataStoreMethod};
use khronos_runtime::utils::khronos_value::{KhronosLazyValue, KhronosValue};
use khronos_runtime::{to_struct, value};
use serde_json::Value;
use serenity::async_trait;
use sqlx::Row;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::str::FromStr;
use uuid::Uuid;

pub const EVENT_LIST: [&str; 71] = [
  "APPLICATION_COMMAND_PERMISSIONS_UPDATE", // Application command permission was updated
  "AUTO_MODERATION_RULE_CREATE", // Auto Moderation rule was created
  "AUTO_MODERATION_RULE_UPDATE", // Auto Moderation rule was updated
  "AUTO_MODERATION_RULE_DELETE", // Auto Moderation rule was deleted
  "AUTO_MODERATION_ACTION_EXECUTION", // Auto Moderation rule was triggered and an action was executed (e.g. a message was blocked)
  "CHANNEL_CREATE", // New guild channel created
  "CHANNEL_UPDATE", // Channel was updated
  "CHANNEL_DELETE", // Channel was deleted
  "CHANNEL_PINS_UPDATE", // Message was pinned or unpinned
  "THREAD_CREATE", // Thread created, also sent when being added to a private thread
  "THREAD_UPDATE", // Thread was updated
  "THREAD_DELETE", // Thread was deleted
  "THREAD_LIST_SYNC", // Sent when gaining access to a channel, contains all active threads in that channel
  "THREAD_MEMBER_UPDATE", // Thread memberfor the current user was updated
  "THREAD_MEMBERS_UPDATE", // Some user(s) were added to or removed from a thread
  "ENTITLEMENT_CREATE", // Entitlement was created
  "ENTITLEMENT_UPDATE", // Entitlement was updated or renewed
  "ENTITLEMENT_DELETE", // Entitlement was deleted
  "GUILD_UPDATE", // Guild was updated
  "GUILD_AUDIT_LOG_ENTRY_CREATE", // A guild audit log entry was created
  "GUILD_BAN_ADD", // User was banned from a guild
  "GUILD_BAN_REMOVE", // User was unbanned from a guild
  "GUILD_EMOJIS_UPDATE", // Guild emojis were updated
  "GUILD_STICKERS_UPDATE", // Guild stickers were updated
  "GUILD_INTEGRATIONS_UPDATE", // Guild integration was updated
  "GUILD_MEMBER_ADD", // New user joined a guild
  "GUILD_MEMBER_REMOVE", // User was removed from a guild
  "GUILD_MEMBER_UPDATE", // Guild member was updated
  "GUILD_MEMBERS_CHUNK", // Response toRequest Guild Members
  "GUILD_ROLE_CREATE", // Guild role was created
  "GUILD_ROLE_UPDATE", // Guild role was updated
  "GUILD_ROLE_DELETE", // Guild role was deleted
  "GUILD_SCHEDULED_EVENT_CREATE", // Guild scheduled event was created
  "GUILD_SCHEDULED_EVENT_UPDATE", // Guild scheduled event was updated
  "GUILD_SCHEDULED_EVENT_DELETE", // Guild scheduled event was deleted
  "GUILD_SCHEDULED_EVENT_USER_ADD", // User subscribed to a guild scheduled event
  "GUILD_SCHEDULED_EVENT_USER_REMOVE", // User unsubscribed from a guild scheduled event
  "GUILD_SOUNDBOARD_SOUND_CREATE", // Guild soundboard sound was created
  "GUILD_SOUNDBOARD_SOUND_UPDATE", // Guild soundboard sound was updated
  "GUILD_SOUNDBOARD_SOUND_DELETE", // Guild soundboard sound was deleted
  "GUILD_SOUNDBOARD_SOUNDS_UPDATE", // Guild soundboard sounds were updated
  "SOUNDBOARD_SOUNDS", // Response toRequest Soundboard Sounds
  "INTEGRATION_CREATE", // Guild integration was created
  "INTEGRATION_UPDATE", // Guild integration was updated
  "INTEGRATION_DELETE", // Guild integration was deleted
  "INTERACTION_CREATE", // User used an interaction, such as anApplication Command
  "INVITE_CREATE", // Invite to a channel was created
  "INVITE_DELETE", // Invite to a channel was deleted
  "MESSAGE", // Message was created
  "MESSAGE_UPDATE", // Message was edited
  "MESSAGE_DELETE", // Message was deleted
  "MESSAGE_DELETE_BULK", // Multiple messages were deleted at once
  "MESSAGE_REACTION_ADD", // User reacted to a message
  "MESSAGE_REACTION_REMOVE", // User removed a reaction from a message
  "MESSAGE_REACTION_REMOVE_ALL", // All reactions were explicitly removed from a message
  "MESSAGE_REACTION_REMOVE_EMOJI", // All reactions for a given emoji were explicitly removed from a message
  "PRESENCE_UPDATE", // User was updated
  "STAGE_INSTANCE_CREATE", // Stage instance was created
  "STAGE_INSTANCE_UPDATE", // Stage instance was updated
  "STAGE_INSTANCE_DELETE", // Stage instance was deleted or closed
  "SUBSCRIPTION_CREATE", // Premium App Subscription was created
  "SUBSCRIPTION_UPDATE", // Premium App Subscription was updated
  "SUBSCRIPTION_DELETE", // Premium App Subscription was deleted
  "TYPING_START", // User started typing in a channel
  "USER_UPDATE", // Properties about the user changed
  "VOICE_CHANNEL_EFFECT_SEND", // Someone sent an effect in a voice channel the current user is connected to
  "VOICE_STATE_UPDATE", // Someone joined, left, or moved a voice channel
  "VOICE_SERVER_UPDATE", // Guild's voice server was updated
  "WEBHOOKS_UPDATE", // Guild channel webhook was created, update, or deleted
  "MESSAGE_POLL_VOTE_ADD", // User voted on a poll
  "MESSAGE_POLL_VOTE_REMOVE", // User removed a vote on a poll
];

/// A data store to expose Anti-Raid's statistics (type in discord /"stats")
pub struct StatsStore {
    pub guild_state: Rc<GuildState>, // reference counted
}

#[async_trait(?Send)]
impl DataStoreImpl for StatsStore {
    fn name(&self) -> String {
        "StatsStore".to_string()
    }

    fn need_caps(&self, _method: &str) -> bool {
        // for security all methods require capabilities (string template metadata)
        false
    }

    fn methods(&self) -> Vec<String> {
        vec!["stats".to_string()]
    }

    fn get_method(&self, key: String) -> Option<DataStoreMethod> {
        if key == "stats" {
            let guild_state_os = self.guild_state.clone();
            Some(DataStoreMethod::Async(Rc::new(move |_v| {
                let guild_state = guild_state_os.clone();
                Box::pin(async move {
                    let total_guilds = {
                        let sandwich_resp =
                            crate::sandwich::get_status(&guild_state.reqwest_client).await?;

                        let mut guild_count = 0;
                        sandwich_resp.shard_conns.iter().for_each(|(_, sc)| {
                            guild_count += sc.guilds;
                        });

                        guild_count
                    };

                    Ok(value!(
                        "total_cached_guilds".to_string() => total_guilds, // This field is deprecated, use total_guilds instead
                        "total_guilds".to_string() => total_guilds,
                        "total_users".to_string() => 0, // for now
                        "last_started_at".to_string() => crate::CONFIG.start_time
                    ))
                })
            })))
        } else {
            None
        }
    }
}

/// A data store to expose Anti-Raid's core links
pub struct LinksStore {}

#[async_trait(?Send)]
impl DataStoreImpl for LinksStore {
    fn name(&self) -> String {
        "LinksStore".to_string()
    }

    fn need_caps(&self, _method: &str) -> bool {
        false
    }

    fn methods(&self) -> Vec<String> {
        vec!["links".to_string(), "event_list".to_string()]
    }

    fn get_method(&self, key: String) -> Option<DataStoreMethod> {
        match key.as_str() {
            "links" => Some(DataStoreMethod::Sync(Rc::new(move |_v| {
                let support_server = crate::CONFIG.meta.support_server_invite.clone();
                let api_url = crate::CONFIG.sites.api.clone();
                let frontend_url = crate::CONFIG.sites.frontend.clone();
                let docs_url = crate::CONFIG.sites.docs.clone();
                Ok(value!(
                    "support_server".to_string() => support_server,
                    "api_url".to_string() => api_url,
                    "frontend_url".to_string() => frontend_url,
                    "docs_url".to_string() => docs_url
                ))
            }))),
            "event_list" => Some(DataStoreMethod::Sync(Rc::new(move |_v| {
                let mut vec = AntiraidEvent::variant_names()
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<String>>();

                vec.extend(
                    EVENT_LIST
                        .iter()
                        .copied()
                        .map(|x| x.to_string())
                        .collect::<Vec<String>>(),
                );

                Ok(value!(vec))
            }))),
            _ => None,
        }
    }
}

to_struct! {
    pub struct Spawn {
        pub name: String,
        pub data: KhronosValue, // need khronos value to convert to/from lua
        pub create: bool,
        pub execute: bool,
        pub id: Option<String>, // If create is false, this is required
    }
}

to_struct!(
    /// Rust internal/special type to better serialize/speed up embed creation
    #[derive(Clone, PartialEq)]
    pub struct Statuses {
        pub level: String,
        pub msg: String,
        pub ts: f64,
        pub bot_display_ignore: Option<Vec<String>>,
        pub extra_info: IndexMap<String, serde_json::Value>,
    }
);

to_struct! {
    pub struct Job {
        pub id: Uuid,
        pub name: String,
        pub output: Option<Output>,
        pub fields: IndexMap<String, Value>,
        pub statuses: Vec<Statuses>,
        pub guild_id: serenity::all::GuildId,
        pub expiry: Option<chrono::Duration>,
        pub state: String,
        pub resumable: bool,
        pub created_at: chrono::DateTime<Utc>,

        // extra fields
        pub job_path: String,
        pub job_file_path: Option<String>,
    }
}

to_struct! {
    pub struct Output {
        pub filename: String,
        pub perguild: Option<bool>, // Temp flag for migrations
    }
}

to_struct!(
    #[derive(Clone, Debug)]
    pub struct Template {
        pub name: String,
        pub events: Vec<String>,
        pub error_channel: Option<String>,
        pub content: KhronosLazyValue,
        pub language: String,
        pub allowed_caps: Vec<String>,
        pub created_at: chrono::DateTime<chrono::Utc>,
        pub updated_at: chrono::DateTime<chrono::Utc>,
        pub paused: bool,
    }
);

to_struct!(
    #[derive(Clone, Debug, Default)]
    pub struct CreateTemplate {
        pub name: String,
        pub events: Vec<String>,
        pub error_channel: Option<String>,
        pub content: HashMap<String, String>,
        pub language: String,
        pub allowed_caps: Vec<String>,
        pub paused: bool,
    }
);

/// Internal representation of a template in postgres
#[derive(sqlx::FromRow)]
struct TemplateData {
    name: String,
    content: serde_json::Value,
    language: String,
    allowed_caps: Vec<String>,
    events: Vec<String>,
    error_channel: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    last_updated_at: chrono::DateTime<chrono::Utc>,
    paused: bool,
}

#[derive(Clone)]
/// A data store to expose template management
pub struct TemplateStore {
    pub guild_state: Rc<GuildState>, // reference counted
}

impl TemplateStore {
    /// Validate the error channel provided for the template
    async fn validate_error_channel(
        &self,
        channel_id: serenity::all::GenericChannelId,
    ) -> Result<(), crate::Error> {
        // Perform required checks
        let Some(channel_json) = crate::sandwich::channel(
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            Some(self.guild_state.guild_id),
            channel_id,
        )
        .await?
        else {
            return Err(format!("Could not find channel with id: {}", channel_id).into());
        };

        let channel = serde_json::from_value::<serenity::all::Channel>(channel_json)
            .map_err(|e| format!("Failed to parse channel: {}", e))?;

        let Some(guild_channel) = channel.guild() else {
            return Err(format!("Channel with id {} is not in a guild", channel_id).into());
        };

        if guild_channel.base.guild_id != self.guild_state.guild_id {
            return Err(
                format!("Channel with id {} is not in the current guild", channel_id).into(),
            );
        }

        let data = self.guild_state.serenity_context.data::<crate::Data>();
        let bot_user_id = data.current_user.id;

        let bot_user = crate::sandwich::member_in_guild(
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            self.guild_state.guild_id,
            bot_user_id,
        )
        .await
        .map_err(|e| format!("Failed to get bot user: {}", e))?;

        let Some(bot_user_json) = bot_user else {
            return Err(format!("Could not find bot user: {}", bot_user_id).into());
        };

        let bot_user = serde_json::from_value::<serenity::all::Member>(bot_user_json)
            .map_err(|e| format!("Failed to parse bot user: {}", e))?;

        let guild_json = crate::sandwich::guild(
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            self.guild_state.guild_id,
        )
        .await
        .map_err(|e| format!("Failed to get guild: {}", e))?;

        let guild = serde_json::from_value::<serenity::all::PartialGuild>(guild_json)
            .map_err(|e| format!("Failed to parse guild: {}", e))?;

        let permissions = guild.user_permissions_in(&guild_channel, &bot_user);

        if !permissions.contains(serenity::all::Permissions::SEND_MESSAGES) {
            return Err(format!(
                "Bot does not have permission to `Send Messages` in channel with id: {}",
                channel_id
            )
            .into());
        }

        Ok(())
    }

    async fn validate_name(&self, name: &str) -> Result<(), crate::Error> {
        if name.starts_with("$shop/") {
            let (shop_tname, shop_tversion) =
                crate::templatingrt::template::Template::parse_shop_template(name)
                    .map_err(|e| format!("Failed to parse shop template: {:?}", e))?;

            let shop_template_count =
                sqlx::query("SELECT COUNT(*) FROM template_shop WHERE name = $1 AND version = $2")
                    .bind(shop_tname)
                    .bind(shop_tversion)
                    .fetch_one(&self.guild_state.pool)
                    .await
                    .map_err(|e| format!("Failed to get shop template: {:?}", e))?
                    .try_get::<Option<i64>, _>(0)
                    .map_err(|e| format!("Failed to get count: {:?}", e))?
                    .unwrap_or_default();

            if shop_template_count == 0 {
                return Err("Shop template does not exist".into());
            }
        }

        Ok(())
    }

    async fn does_template_exist(&self, name: &str) -> Result<bool, crate::Error> {
        let count =
            sqlx::query("SELECT COUNT(*) FROM guild_templates WHERE guild_id = $1 AND name = $2")
                .bind(self.guild_state.guild_id.to_string())
                .bind(name)
                .fetch_one(&self.guild_state.pool)
                .await
                .map_err(|e| format!("Failed to check if template exists: {:?}", e))?
                .try_get::<i64, _>(0)
                .map_err(|e| format!("Failed to get count: {:?}", e))?;

        Ok(count > 0)
    }
}

#[async_trait(?Send)]
impl DataStoreImpl for TemplateStore {
    fn name(&self) -> String {
        "TemplateStore".to_string()
    }

    fn need_caps(&self, _method: &str) -> bool {
        true
    }

    fn methods(&self) -> Vec<String> {
        vec![
            "list".to_string(),
            "get".to_string(),
            "create".to_string(),
            "update".to_string(),
            "delete".to_string(),
            "start".to_string(), // Start a template with a OnStartup
        ]
    }

    fn get_method(&self, key: String) -> Option<DataStoreMethod> {
        match key.as_str() {
            "list" => {
                let guild_state_ref = self.guild_state.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |_v| {
                    let guild_state = guild_state_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let templates: Vec<TemplateData> = sqlx::query_as(
                            "SELECT name, content, language, allowed_caps, events, error_channel, paused, created_at, last_updated_at FROM guild_templates WHERE guild_id = $1",
                        )
                        .bind(guild_state.guild_id.to_string())
                        .fetch_all(&guild_state.pool)
                        .await?;

                        let mut result = Vec::with_capacity(templates.len());

                        for template in templates {
                            result.push(Template {
                                name: template.name,
                                events: template.events,
                                error_channel: template.error_channel,
                                content: KhronosLazyValue {
                                    data: template.content,
                                },
                                language: template.language,
                                allowed_caps: template.allowed_caps,
                                created_at: template.created_at,
                                updated_at: template.last_updated_at,
                                paused: template.paused,
                            });
                        }

                        Ok(value!(result))
                    })
                })))
            }
            "get" => {
                let guild_state_ref = self.guild_state.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let guild_state = guild_state_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = VecDeque::from(v);

                        let Some(KhronosValue::Text(name)) = v.pop_front() else {
                            return Err(
                                "arg #1 to TemplateStore.get must be a string (name)".into()
                            );
                        };

                        let templates: Option<TemplateData> = sqlx::query_as(
                            "SELECT name, content, language, allowed_caps, events, error_channel, paused, created_at, last_updated_at FROM guild_templates WHERE guild_id = $1 AND name = $2",
                        )
                        .bind(guild_state.guild_id.to_string())
                        .bind(name)
                        .fetch_optional(&guild_state.pool)
                        .await?;

                        let Some(template) = templates else {
                            return Ok(value!(KhronosValue::Null));
                        };

                        let template = Template {
                            name: template.name,
                            events: template.events,
                            error_channel: template.error_channel,
                            content: KhronosLazyValue {
                                data: template.content,
                            },
                            language: template.language,
                            allowed_caps: template.allowed_caps,
                            created_at: template.created_at,
                            updated_at: template.last_updated_at,
                            paused: template.paused,
                        };

                        Ok(value!(template))
                    })
                })))
            }
            "create" => {
                let self_ref = self.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let self_ref = self_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = VecDeque::from(v);

                        let Some(data) = v.pop_front() else {
                            // first arg b/c rust creates internal lua func
                            return Err("arg #1 of spawn data is missing".into());
                        };

                        let create_template: CreateTemplate = data.try_into()?;

                        if self_ref.does_template_exist(&create_template.name).await? {
                            return Err("Template already exists".into());
                        }

                        self_ref.validate_name(&create_template.name).await?;

                        if let Some(error_channel) = &create_template.error_channel {
                            let channel_id: serenity::all::GenericChannelId = error_channel
                                .parse()
                                .map_err(|e| format!("Failed to parse error channel: {:?}", e))?;

                            self_ref
                                .validate_error_channel(channel_id)
                                .await
                                .map_err(|e| format!("Failed to validate error channel: {}", e))?;
                        }

                        TemplateLanguage::from_str(&create_template.language)
                            .map_err(|e| format!("Failed to parse language: {:?}", e))?;

                        sqlx::query(
                            "INSERT INTO guild_templates (guild_id, name, language, content, events, paused, allowed_caps, error_channel) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                        )
                        .bind(self_ref.guild_state.guild_id.to_string())
                        .bind(&create_template.name)
                        .bind(create_template.language)
                        .bind(serde_json::to_value(create_template.content)?)
                        .bind(&create_template.events)
                        .bind(create_template.paused)
                        .bind(&create_template.allowed_caps)
                        .bind(&create_template.error_channel)
                        .execute(&self_ref.guild_state.pool)
                        .await
                        .map_err(|e| format!("Failed to insert template: {:?}", e))?;

                        DEFERRED_CACHE_REGENS
                            .insert(
                                self_ref.guild_state.guild_id,
                                DeferredCacheRegenMode::FlushSingle {},
                            )
                            .await;

                        Ok(value!(KhronosValue::Null))
                    })
                })))
            }
            "update" => {
                let self_ref = self.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let self_ref = self_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = VecDeque::from(v);

                        let Some(data) = v.pop_front() else {
                            // first arg b/c rust creates internal lua func
                            return Err("arg #1 of spawn data is missing".into());
                        };

                        let create_template: CreateTemplate = data.try_into()?;

                        if !self_ref.does_template_exist(&create_template.name).await? {
                            return Err("Template does not already exist".into());
                        }

                        self_ref.validate_name(&create_template.name).await?;

                        if let Some(error_channel) = &create_template.error_channel {
                            let channel_id: serenity::all::GenericChannelId = error_channel
                                .parse()
                                .map_err(|e| format!("Failed to parse error channel: {:?}", e))?;

                            self_ref
                                .validate_error_channel(channel_id)
                                .await
                                .map_err(|e| format!("Failed to validate error channel: {}", e))?;
                        }

                        TemplateLanguage::from_str(&create_template.language)
                            .map_err(|e| format!("Failed to parse language: {:?}", e))?;

                        sqlx::query(
                            "UPDATE guild_templates SET language = $3, content = $4, events = $5, paused = $6, allowed_caps = $7, error_channel = $8, last_updated_at = NOW() WHERE guild_id = $1 AND name = $2",
                        )
                        .bind(self_ref.guild_state.guild_id.to_string())
                        .bind(&create_template.name)
                        .bind(create_template.language)
                        .bind(serde_json::to_value(create_template.content)?)
                        .bind(&create_template.events)
                        .bind(create_template.paused)
                        .bind(&create_template.allowed_caps)
                        .bind(&create_template.error_channel)
                        .execute(&self_ref.guild_state.pool)
                        .await
                        .map_err(|e| format!("Failed to update template: {:?}", e))?;

                        DEFERRED_CACHE_REGENS
                            .insert(
                                self_ref.guild_state.guild_id,
                                DeferredCacheRegenMode::FlushSingle {},
                            )
                            .await;

                        Ok(value!(KhronosValue::Null))
                    })
                })))
            }
            "delete" => {
                let self_ref = self.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let self_ref = self_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = VecDeque::from(v);

                        let Some(name) = v.pop_front() else {
                            return Err("arg #1 of TemplateStore.delete is missing (name)".into());
                        };

                        let name: String = match name {
                            KhronosValue::Text(name) => {
                                if name.is_empty() {
                                    return Err(
                                        "arg #1 of TemplateStore.delete must not be empty (name)"
                                            .into(),
                                    );
                                }
                                name
                            }
                            _ => {
                                return Err(
                                    "arg #1 to TemplateStore.delete must be a string (name)".into(),
                                )
                            }
                        };

                        if !self_ref.does_template_exist(&name).await? {
                            return Err("Template does not exist".into());
                        }

                        sqlx::query(
                            "DELETE FROM guild_templates WHERE guild_id = $1 AND name = $2",
                        )
                        .bind(self_ref.guild_state.guild_id.to_string())
                        .bind(&name)
                        .execute(&self_ref.guild_state.pool)
                        .await
                        .map_err(|e| format!("Failed to delete template: {:?}", e))?;

                        DEFERRED_CACHE_REGENS
                            .insert(
                                self_ref.guild_state.guild_id,
                                DeferredCacheRegenMode::FlushSingle {},
                            )
                            .await;

                        Ok(value!(KhronosValue::Null))
                    })
                })))
            }
            "start" => {
                let self_ref = self.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let self_ref = self_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = VecDeque::from(v);

                        let Some(name) = v.pop_front() else {
                            return Err("arg #1 of TemplateStore.start is missing (name)".into());
                        };

                        let name: String = match name {
                            KhronosValue::Text(name) => {
                                if name.is_empty() {
                                    return Err(
                                        "arg #1 of TemplateStore.start must not be empty (name)"
                                            .into(),
                                    );
                                }
                                name
                            }
                            _ => {
                                return Err(
                                    "arg #1 to TemplateStore.start must be a string (name)".into(),
                                )
                            }
                        };

                        if !self_ref.does_template_exist(&name).await? {
                            return Err("Template does not exist".into());
                        }

                        let data = self_ref.guild_state.serenity_context.data::<crate::Data>();
                        let v = crate::dispatch::dispatch_to_template_and_wait::<KhronosValueResponse>(
                            &self_ref.guild_state.serenity_context,
                            &crate::data::Data {
                                pool: self_ref.guild_state.pool.clone(),
                                reqwest: self_ref.guild_state.reqwest_client.clone(),
                                object_store: self_ref.guild_state.object_store.clone(),
                                current_user: data.current_user.clone(),
                            },
                            parse_event(&AntiraidEvent::KeyResume(KeyResumeEvent {
                                id: "START_TEMPLATE".to_string(),
                                key: "START_TEMPLATE".to_string(),
                                scopes: vec![],
                            }))?,
                            self_ref.guild_state.guild_id,
                            MAX_TEMPLATES_RETURN_WAIT_TIME,
                            &name
                        )
                        .await
                        .map_err(|e| format!("Failed to start template: {:?}", e))?;

                        KhronosValueMapper(v).into_khronos_value()
                    })
                })))
            }
            _ => None,
        }
    }
}
