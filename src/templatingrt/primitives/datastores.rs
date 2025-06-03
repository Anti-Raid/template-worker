use indexmap::IndexMap;
use serenity::async_trait;
use uuid::Uuid;
use std::rc::Rc;
use khronos_runtime::{value, to_struct};
use khronos_runtime::utils::khronos_value::KhronosValue;
use khronos_runtime::traits::ir::{DataStoreImpl, DataStoreMethod};
use crate::templatingrt::state::GuildState;
use super::sandwich_config;
use crate::config::CONFIG;
use chrono::Utc;

/// A data store to expose Anti-Raid's statistics (type in discord /"stats")
pub struct StatsStore {
    pub guild_state: Rc<GuildState>, // reference counted
}

#[async_trait(?Send)]
impl DataStoreImpl for StatsStore {
    fn name(&self) -> String {
        "StatsStore".to_string()
    }

    fn need_caps(&self, _method: &str) -> bool { // for security all methods require capabilities (string template metadata)
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
                            sandwich_driver::get_status(&guild_state.reqwest_client, &sandwich_config()).await?;
                
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
                Ok(value!(
                    "support_server".to_string() => support_server
                ))
            }))),
            _ => None,
        }
    }
}

/// A data store to expose job server
pub struct JobServerStore {
    pub guild_state: Rc<GuildState>, // reference counted
}

to_struct! {
    pub struct Spawn {
        pub name: String,
        pub data: KhronosValue, // need khronos value to convert to/from lua
        pub create: bool,
        pub execute: bool,
        pub id: Option<String>, // If create is false, this is required
        pub user_id: String,
    }
}

/// Rust internal/special type to better serialize/speed up embed creation
#[derive(serde::Serialize, serde::Deserialize, Clone, PartialEq)]
pub struct Statuses {
    pub level: String,
    pub msg: String,
    pub ts: f64,
    #[serde(rename = "botDisplayIgnore")]
    pub bot_display_ignore: Option<Vec<String>>,

    #[serde(flatten)]
    pub extra_info: IndexMap<String, serde_json::Value>,
}

impl TryFrom<KhronosValue> for Statuses {
    type Error = silverpelt::Error;
    fn try_from(value: KhronosValue) -> Result<Self, Self::Error> {
        value.into_value()
    }
}

impl TryFrom<Statuses> for KhronosValue {
    type Error = silverpelt::Error;
    fn try_from(value: Statuses) -> Result<Self, Self::Error> {
        KhronosValue::from_serde_json_value(serde_json::to_value(value)?, 0)
    }
}

to_struct! {
    pub struct Job {
        pub id: Uuid,
        pub name: String,
        pub output: Option<Output>,
        pub fields: std::collections::HashMap<String, KhronosValue>,
        pub statuses: Vec<Statuses>,
        pub guild_id: serenity::all::GuildId,
        pub expiry: Option<chrono::Duration>,
        pub state: String,
        pub resumable: bool,
        pub created_at: chrono::DateTime<Utc>,
    }
}

to_struct! {
    pub struct Output {
        pub filename: String,
        pub perguild: Option<bool>, // Temp flag for migrations
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
        vec!["spawn".to_string(), "list".to_string(), "get".to_string(), "delete".to_string()]
    }

    fn get_method(&self, key: String) -> Option<DataStoreMethod> {
        match key.as_str() {
            "spawn" => { // used to call method in jobserver
                let guild_state_ref = self.guild_state.clone(); // reference to the guild state data
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let guild_state = guild_state_ref.clone(); // satisfy rusts borrowing rules
                    Box::pin(async move { // doesn't move around in memory; doesn't block other vms
                        let mut v = v;
                        let Some(spawn_data) = v.pop() else { // first arg b/c rust creates internal lua func
                            return Err("arg #1 of spawn data is missing".into());
                        };

                        let spawn: Spawn = spawn_data.try_into()?;

                        let js_spawn = jobserver::Spawn {
                            name: spawn.name,
                            data: spawn.data.into_serde_json_value(0, false)?,
                            create: spawn.create,
                            execute: spawn.execute,
                            id: spawn.id,
                            user_id: spawn.user_id,
                        };

                        let resp = jobserver::spawn::spawn_task(
                            &guild_state.reqwest_client,
                            &js_spawn,
                            &CONFIG.base_ports.jobserver_base_addr,
                            CONFIG.base_ports.jobserver,
                        )
                        .await?;

                        Ok(value!(resp.id))
                    })
                })))
            },
            "list" => { 
                  None
            },
            "get" => { 
                None
            },
            "delete" => { 
                None
            },
            _ => None,
        }
    }
}
