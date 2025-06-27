use crate::config::CONFIG;
use crate::jobserver;
use crate::templatingrt::state::GuildState;
use chrono::Utc;
use indexmap::IndexMap;
use khronos_runtime::traits::ir::{DataStoreImpl, DataStoreMethod};
use khronos_runtime::utils::khronos_value::KhronosValue;
use khronos_runtime::{to_struct, value};
use serde_json::Value;
use serenity::async_trait;
use std::collections::VecDeque;
use std::rc::Rc;
use uuid::Uuid;

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
        vec!["links".to_string()]
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
