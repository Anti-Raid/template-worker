use serenity::async_trait;
use std::rc::Rc;
use khronos_runtime::value;
use khronos_runtime::traits::ir::{DataStoreImpl, DataStoreMethod};
use crate::templatingrt::state::GuildState;
use super::sandwich_config;

/// A data store to expose Anti-Raid's statistics
pub struct StatsStore {
    pub guild_state: Rc<GuildState>,
}

#[async_trait(?Send)]
impl DataStoreImpl for StatsStore {
    fn name(&self) -> String {
        "StatsStore".to_string()
    }

    fn need_caps(&self, _method: &str) -> bool {
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
                        "total_users".to_string() => total_users
                    ))
                })
            })))
        } else {
            None
        }
    }
}