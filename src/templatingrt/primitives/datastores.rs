use serenity::async_trait;
use std::rc::Rc;
use khronos_runtime::value;
use khronos_runtime::utils::khronos_value::KhronosValue;
use khronos_runtime::traits::ir::{DataStoreImpl, DataStoreMethod};
use crate::templatingrt::state::GuildState;
use super::sandwich_config;
use crate::templatingrt::cache::{get_guild_template, get_all_guild_templates};
use khronos_runtime::primitives::event::Event;

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

/// A data store to trigger an OnStartup/ExternalKeyUpdate event in another template
pub struct TriggerStore {
    pub guild_state: Rc<GuildState>,
    pub manager: khronos_runtime::rt::KhronosRuntimeManager
}

#[async_trait(?Send)]
impl DataStoreImpl for TriggerStore {
    fn name(&self) -> String {
        "TriggerStore".to_string()
    }

    fn need_caps(&self, _method: &str) -> bool {
        true // TriggerStore always needs caps
    }

    fn methods(&self) -> Vec<String> {
        vec!["trigger".to_string()]
    }

    fn get_method(&self, key: String) -> Option<DataStoreMethod> {
        match key.as_str() {
            "trigger" => {
                let guild_state_os = self.guild_state.clone();
                let manager_os = self.manager.clone();
                Some(DataStoreMethod::Async(Rc::new(move |v| {
                    let guild_state = guild_state_os.clone();
                    let manager = manager_os.clone();
                    Box::pin(async move {
                        let mut v = v;
                        let Some(KhronosValue::Text(template_name)) = v.pop() else {
                            return Err("arg #1 of template name to trigger is missing".into());
                        };
    
                        let event = match v.pop() {
                            Some(data) => {
                                match data.into_value::<antiraid_types::ar_event::AntiraidEvent>() {
                                    Ok(e) => {
                                        match e {
                                            antiraid_types::ar_event::AntiraidEvent::OnStartup(_) | antiraid_types::ar_event::AntiraidEvent::ExternalKeyUpdate(_) => {},
                                            _ => {
                                                return Err("arg #2 of event name to trigger is not a valid event (only OnStartup and ExternalKeyUpdate can be triggered using a TriggerDataStore)".into());
                                            }
                                        };
    
                                        e
                                    }
                                    Err(_) => {
                                        return Err("arg #2 of event name to trigger is not a valid event".into());
                                    }
                                }
                            }
                            None => {
                                antiraid_types::ar_event::AntiraidEvent::OnStartup(vec![])
                            }
                        };    

                        let event = crate::dispatch::parse_event(
                            &event,
                        )?;

                        if template_name.is_empty() {
                            // Dispatch to all templates
                            let Some(templates) = get_all_guild_templates(guild_state.guild_id).await else {
                                return Err("No templates found".into());
                            };

                            let res = crate::templatingrt::vm_manager::dispatch_event_to_multiple_templates(
                                templates,
                                event,
                                &manager,
                                guild_state
                            ).await;

                            let mut results = vec![];

                            for (name, res) in res {
                                match res {
                                    crate::templatingrt::vm_manager::LuaVmResult::Ok { result_val } => {
                                        results.push(value!(name => KhronosValue::from_serde_json_value(serde_json::json!({
                                                "error": false,
                                                "result": result_val
                                            }), 0)
                                            .unwrap_or(value!(name => value!("error".to_string() => true, "result".to_string() => "Failed to convert result to JSON".to_string())))
                                        ));
                                    }
                                    crate::templatingrt::vm_manager::LuaVmResult::LuaError { err } => {
                                        results.push(value!(name => KhronosValue::from_serde_json_value(serde_json::json!({
                                                "error": true,
                                                "result": err.to_string()
                                            }), 0)
                                            .unwrap_or(value!(name => value!("error".to_string() => true, "result".to_string() => "Failed to convert result to JSON".to_string())))
                                        ));
                                    }
                                    crate::templatingrt::vm_manager::LuaVmResult::VmBroken {} => {
                                        results.push(value!(name => KhronosValue::from_serde_json_value(serde_json::json!({
                                            "error": true,
                                            "result": format!("VM for template '{}' is marked as broken", name)
                                            }), 0)
                                            .unwrap_or(value!(name => value!("error".to_string() => true, "result".to_string() => "Failed to convert result to JSON".to_string())))
                                        ));
                                    }
                                }
                            }

                            return Ok(KhronosValue::List(results));
                        } else {
                            // Dispatch to specific template
                            let Some(template) = get_guild_template(guild_state.guild_id, &template_name).await else {
                                return Err(format!("Template '{}' not found", template_name).into());
                            };

                            let res = crate::templatingrt::vm_manager::dispatch_event_to_template(
                                template,
                                Event::from_create_event(&event),
                                manager,
                                guild_state
                            ).await;

                            let mut results = Vec::with_capacity(1);
                            match res {
                                crate::templatingrt::vm_manager::LuaVmResult::Ok { result_val } => {
                                    results.push(value!(template_name => KhronosValue::from_serde_json_value(serde_json::json!({
                                            "error": false,
                                            "result": result_val
                                        }), 0)
                                        .unwrap_or(value!(template_name => value!("error".to_string() => true, "result".to_string() => "Failed to convert result to JSON".to_string())))
                                    ));
                                    return Ok(KhronosValue::List(results));
                                }
                                crate::templatingrt::vm_manager::LuaVmResult::LuaError { err } => {
                                    results.push(value!(template_name => KhronosValue::from_serde_json_value(serde_json::json!({
                                        "error": true,
                                        "result": err.to_string()
                                        }), 0)
                                        .unwrap_or(value!(template_name => value!("error".to_string() => true, "result".to_string() => "Failed to convert result to JSON".to_string())))
                                    ));
                                    return Ok(KhronosValue::List(results));
                                }
                                crate::templatingrt::vm_manager::LuaVmResult::VmBroken {} => {
                                    results.push(value!(template_name => KhronosValue::from_serde_json_value(serde_json::json!({
                                        "error": true,
                                        "result": format!("VM for template '{}' is marked as broken", template_name)
                                        }), 0)
                                        .unwrap_or(value!(template_name => value!("error".to_string() => true, "result".to_string() => "Failed to convert result to JSON".to_string())))
                                    ));
                                    return Ok(KhronosValue::List(results));
                                }
                            }
                        }
                    })
                })))
            },
            _ => None,
        }
    }
}