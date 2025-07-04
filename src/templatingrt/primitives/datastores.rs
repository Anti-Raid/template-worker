use crate::config::CONFIG;
use crate::jobserver;
use crate::templatingrt::cache::{DeferredCacheRegenMode, DEFERRED_CACHE_REGENS};
use crate::templatingrt::state::GuildState;
use crate::templatingrt::template::TemplateLanguage;
use antiraid_types::ar_event::AntiraidEvent;
use chrono::Utc;
use indexmap::IndexMap;
use khronos_runtime::traits::ir::{DataStoreImpl, DataStoreMethod};
use khronos_runtime::utils::khronos_value::KhronosValue;
use khronos_runtime::{to_struct, value};
use serde_json::Value;
use serenity::async_trait;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::str::FromStr;
use uuid::Uuid;
use sqlx::Row;

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
                    let ctx = &guild_state.serenity_context;
                    let total_cached_guilds = ctx.cache.guild_count();

                    let total_guilds = {
                        let sandwich_resp =
                            crate::sandwich::get_status(&guild_state.reqwest_client).await?;

                        let mut guild_count = 0;
                        sandwich_resp.shard_conns.iter().for_each(|(_, sc)| {
                            guild_count += sc.guilds;
                        });

                        guild_count
                    };

                    let total_users = {
                        let mut count = 0;

                        for guild in ctx.cache.guilds() {
                            {
                                let guild = guild.to_guild_cached(&ctx.cache);

                                if let Some(guild) = guild {
                                    count += guild.member_count;
                                }
                            }
                        }

                        count
                    };

                    Ok(value!(
                        "total_cached_guilds".to_string() => total_cached_guilds,
                        "total_guilds".to_string() => total_guilds,
                        "total_users".to_string() => total_users,
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
                    gwevent::core::event_list()
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

/// A data store to expose job server
pub struct JobServerStore {
    pub guild_state: Rc<GuildState>, // reference counted
}

impl JobServerStore {
    /// Converts a ``jobserver::Job`` to a ``Job``
    fn convert_job(j: jobserver::Job, needs_statuses: bool) -> Job {
        Job {
            // extra fields
            job_path: j.get_path(),
            job_file_path: j.get_file_path(),

            // normal fields
            id: j.id,
            name: j.name,
            output: j.output.map(|o| Output {
                filename: o.filename,
                perguild: o.perguild,
            }),
            fields: j.fields,
            statuses: {
                if needs_statuses {
                    j.statuses
                        .into_iter()
                        .map(|s| Statuses {
                            level: s.level,
                            msg: s.msg,
                            ts: s.ts,
                            bot_display_ignore: s.bot_display_ignore,
                            extra_info: s.extra_info,
                        })
                        .collect::<Vec<_>>()
                } else {
                    Vec::with_capacity(0)
                }
            },
            guild_id: j.guild_id,
            expiry: j.expiry,
            state: j.state,
            resumable: j.resumable,
            created_at: j.created_at,
        }
    }
}

#[async_trait(?Send)]
impl DataStoreImpl for JobServerStore {
    fn name(&self) -> String {
        "JobServerStore".to_string()
    }

    fn need_caps(&self, _method: &str) -> bool {
        true
    }

    fn methods(&self) -> Vec<String> {
        vec![
            "spawn".to_string(),
            "list".to_string(),
            "list_named".to_string(),
            "get".to_string(),
            "delete".to_string(),
        ]
    }

    fn get_method(&self, key: String) -> Option<DataStoreMethod> {
        match key.as_str() {
            "spawn" => {
                // used to call method in jobserver
                let guild_state_ref = self.guild_state.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let guild_state = guild_state_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        // doesn't move around in memory; doesn't block other vms
                        let mut v = v;
                        let Some(spawn_data) = v.pop() else {
                            // first arg b/c rust creates internal lua func
                            return Err("arg #1 of spawn data is missing".into());
                        };

                        let spawn: Spawn = spawn_data.try_into()?;

                        let js_spawn = jobserver::Spawn {
                            name: spawn.name,
                            data: spawn.data.into_serde_json_value(0, false)?,
                            create: spawn.create,
                            execute: spawn.execute,
                            id: spawn.id,
                            guild_id: guild_state.guild_id.to_string(),
                        };

                        let resp = jobserver::spawn_task(
                            &guild_state.reqwest_client,
                            &js_spawn,
                            &CONFIG.base_ports.jobserver_base_addr,
                            CONFIG.base_ports.jobserver,
                        )
                        .await?;

                        Ok(value!(resp.id))
                    })
                })))
            }
            "list" => {
                let guild_state_ref = self.guild_state.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let guild_state = guild_state_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = v;

                        let mut needs_statuses = false;
                        if let Some(val) = v.pop() {
                            match val {
                                KhronosValue::Boolean(b) => needs_statuses = b,
                                _ => {
                                    return Err(
                                        "arg #1 to JobServerStore.list must be a boolean (needs_statuses)".into()
                                    )
                                }
                            }
                        }

                        // doesn't move around in memory; doesn't block other vms
                        let jobs =
                            jobserver::Job::from_guild(guild_state.guild_id, &guild_state.pool)
                                .await?
                                .into_iter()
                                .map(|j| Self::convert_job(j, needs_statuses))
                                .collect::<Vec<_>>();

                        Ok(value!(jobs))
                    })
                })))
            }
            "list_named" => {
                let guild_state_ref = self.guild_state.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let guild_state = guild_state_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = VecDeque::from(v);

                        let Some(name) = v.pop_front() else {
                            return Err(
                                "arg #1 of JobServerStore.list_named is missing (name)".into()
                            );
                        };

                        let name = match name {
                            KhronosValue::Text(name) => {
                                if name.is_empty() {
                                    return Err("arg #1 of JobServerStore.list_named must not be empty (name)".into());
                                }

                                name
                            }
                            _ => {
                                return Err(
                                    "arg #1 to JobServerStore.list_named must be a string (name)"
                                        .into(),
                                )
                            }
                        };

                        let mut needs_statuses = false;
                        if let Some(val) = v.pop_front() {
                            match val {
                                KhronosValue::Boolean(b) => needs_statuses = b,
                                _ => {
                                    return Err(
                                        "arg #2 to JobServerStore.list_named must be a boolean"
                                            .into(),
                                    )
                                }
                            }
                        }

                        // doesn't move around in memory; doesn't block other vms
                        let jobs = jobserver::Job::from_guild_and_name(
                            guild_state.guild_id,
                            &name,
                            &guild_state.pool,
                        )
                        .await?
                        .into_iter()
                        .map(|j| Self::convert_job(j, needs_statuses))
                        .collect::<Vec<_>>();

                        Ok(value!(jobs))
                    })
                })))
            }
            "get" => {
                let guild_state_ref = self.guild_state.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let guild_state = guild_state_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = VecDeque::from(v);

                        let Some(job_id) = v.pop_front() else {
                            return Err("arg #1 of JobServerStore.get is missing (job_id)".into());
                        };

                        let mut need_statuses = false;
                        if let Some(val) = v.pop_front() {
                            match val {
                                KhronosValue::Boolean(b) => need_statuses = b,
                                _ => {
                                    return Err(
                                        format!("arg #2 to JobServerStore.get must be a boolean (need_statuses), got {:?} and vals {:?}", val, v).into()
                                    )
                                }
                            }
                        }

                        let job_id: Uuid = job_id.try_into()?;

                        let job = jobserver::Job::from_id(job_id, &guild_state.pool).await?;

                        if job.guild_id != guild_state.guild_id {
                            return Err("Job does not belong to this guild".into());
                        }

                        let job = Self::convert_job(job, need_statuses);

                        Ok(value!(job))
                    })
                })))
            }
            "delete" => {
                let guild_state_ref = self.guild_state.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let guild_state = guild_state_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move {
                        let mut v = v;

                        let Some(job_id) = v.pop() else {
                            return Err(
                                "arg #1 of JobServerStore.delete is missing (job_id)".into()
                            );
                        };

                        let job_id: Uuid = job_id.try_into()?;

                        let job = jobserver::Job::from_id(job_id, &guild_state.pool).await?;

                        if job.guild_id != guild_state.guild_id {
                            return Err("Job does not belong to this guild".into());
                        }

                        job.delete(&guild_state.pool, &guild_state.object_store)
                            .await?;

                        Ok(KhronosValue::Null)
                    })
                })))
            }
            _ => None,
        }
    }
}

to_struct!(
    #[derive(Clone, Debug, Default)]
    pub struct Template {
        pub name: String,
        pub events: Vec<String>,
        pub error_channel: Option<String>,
        pub content: HashMap<String, String>,
        pub lang: String,
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
        pub lang: String,
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
    async fn validate_error_channel(&self, channel_id: serenity::all::ChannelId) -> Result<(), crate::Error> {
        // Perform required checks
        let Some(channel) = crate::sandwich::channel(
            &self.guild_state.serenity_context.cache,
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            Some(self.guild_state.guild_id),
            channel_id,
        )
        .await? else {
            return Err(format!("Could not find channel with id: {}", channel_id).into());
        };

        let Some(guild_channel) = channel.guild() else {
            return Err(format!("Channel with id {} is not in a guild", channel_id).into());
        };

        if guild_channel.guild_id != self.guild_state.guild_id {
            return Err(format!("Channel with id {} is not in the current guild", channel_id).into());
        }

        let bot_user_id = self.guild_state.serenity_context.cache.current_user().id;

        let bot_user = crate::sandwich::member_in_guild(
            &self.guild_state.serenity_context.cache,
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            self.guild_state.guild_id,
            bot_user_id,
        )
        .await
        .map_err(|e| format!("Failed to get bot user: {}", e))?;

        let Some(bot_user) = bot_user else {
            return Err(format!("Could not find bot user: {}", bot_user_id).into());
        };

        let guild = crate::sandwich::guild(
            &self.guild_state.serenity_context.cache,
            &self.guild_state.serenity_context.http,
            &self.guild_state.reqwest_client,
            self.guild_state.guild_id,
        )
        .await
        .map_err(|e| format!("Failed to get guild: {}", e))?;

        let permissions = guild.user_permissions_in(&guild_channel, &bot_user);

        if !permissions.contains(serenity::all::Permissions::SEND_MESSAGES) {
            return Err(
                format!("Bot does not have permission to `Send Messages` in channel with id: {}", channel_id).into()
            );
        }

        Ok(())
    }

    async fn validate_name(&self, name: &str) -> Result<(), crate::Error> {
        if name.starts_with("$shop/") {
            let (shop_tname, shop_tversion) = crate::templatingrt::template::Template::parse_shop_template(name)
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
        let count = sqlx::query("SELECT COUNT(*) FROM guild_templates WHERE guild_id = $1 AND name = $2")
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
            "create".to_string(),
            "update".to_string(),
            "delete".to_string(),
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
                            "SELECT name, content, language, allowed_caps, events, error_channel, created_at, created_by, last_updated_at, last_updated_by FROM guild_templates WHERE guild_id = $1 AND paused = false",
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
                                content: serde_json::from_value(template.content)
                                    .map_err(|e| format!("Failed to parse content: {}", e))?,
                                lang: template.language,
                                allowed_caps: template.allowed_caps,
                                created_at: template.created_at,
                                updated_at: template.last_updated_at,
                                paused: template.paused,
                            });
                        }

                        Ok(value!(result))
                    })
                })))
            },
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
                            let channel_id: serenity::all::ChannelId = error_channel
                                .parse()
                                .map_err(|e| format!("Failed to parse error channel: {:?}", e))?;

                            self_ref.validate_error_channel(channel_id).await.map_err(|e| {
                                format!("Failed to validate error channel: {}", e)
                            })?;
                        }

                        TemplateLanguage::from_str(&create_template.lang)
                            .map_err(|e| format!("Failed to parse language: {:?}", e))?;

                        sqlx::query(
                            "INSERT INTO guild_templates (guild_id, name, language, content, events, paused, allowed_caps, error_channel) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
                        )
                        .bind(self_ref.guild_state.guild_id.to_string())
                        .bind(&create_template.name)
                        .bind(create_template.lang)
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
                                DeferredCacheRegenMode::OnReady {
                                    modified: vec![create_template.name.to_string()],
                                },
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
                            let channel_id: serenity::all::ChannelId = error_channel
                                .parse()
                                .map_err(|e| format!("Failed to parse error channel: {:?}", e))?;

                            self_ref.validate_error_channel(channel_id).await.map_err(|e| {
                                format!("Failed to validate error channel: {}", e)
                            })?;
                        }

                        TemplateLanguage::from_str(&create_template.lang)
                            .map_err(|e| format!("Failed to parse language: {:?}", e))?;

                        sqlx::query(
                            "UPDATE guild_templates SET language = $3, content = $4, events = $5, paused = $6, allowed_caps = $7, error_channel = $8, last_updated_at = NOW() WHERE guild_id = $1 AND name = $2",
                        )
                        .bind(self_ref.guild_state.guild_id.to_string())
                        .bind(&create_template.name)
                        .bind(create_template.lang)
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
                                DeferredCacheRegenMode::OnReady {
                                    modified: vec![create_template.name.to_string()],
                                },
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
                                    return Err("arg #1 of TemplateStore.delete must not be empty (name)".into());
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
                                DeferredCacheRegenMode::OnReady {
                                    modified: vec![name],
                                },
                            )
                            .await;

                        Ok(value!(KhronosValue::Null))
                    })
                })))
            }
            _ => None,
        }
    }
}

