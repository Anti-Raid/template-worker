use std::rc::Rc;

use super::core::{resolve_template_to_bytecode, ArLuaThreadInnerState, LuaVmAction, LuaVmResult};

use crate::templatingrt::{primitives::ctxprovider::TemplateContextProvider, state::GuildState};

use khronos_runtime::{primitives::event::Event, TemplateContext};
use mlua::prelude::*;
use mlua_scheduler_ext::traits::IntoLuaThread;

/// Handles a Lua VM action, returning a result
pub(super) async fn handle_event(
    action: LuaVmAction,
    tis_ref: &ArLuaThreadInnerState,
    guild_state: Rc<GuildState>,
) -> LuaVmResult {
    match action {
        LuaVmAction::Exec { template, event } => {
            if tis_ref.broken.load(std::sync::atomic::Ordering::Acquire) {
                return LuaVmResult::VmBroken {};
            }

            // Check bytecode cache first, compile template if not found
            let template_bytecode =
                match resolve_template_to_bytecode(&template, &tis_ref.bytecode_cache).await {
                    Ok(bytecode) => bytecode,
                    Err(e) => {
                        return LuaVmResult::LuaError { err: e.to_string() };
                    }
                };

            let thread = match tis_ref
                .lua
                .load(&template_bytecode)
                .set_name(&template.name)
                .set_mode(mlua::ChunkMode::Binary) // Ensure auto-detection never selects binary mode
                .set_environment(tis_ref.global_table.clone())
                .into_lua_thread(&tis_ref.lua)
            {
                Ok(f) => f,
                Err(e) => {
                    // Mark memory error'd VMs as broken automatically to avoid user grief/pain
                    if let LuaError::MemoryError(_) = e {
                        // Mark VM as broken
                        tis_ref
                            .broken
                            .store(true, std::sync::atomic::Ordering::Release);
                    }

                    return LuaVmResult::LuaError { err: e.to_string() };
                }
            };

            // Now, create the template context that should be passed to the template
            let provider = TemplateContextProvider {
                guild_state,
                template_data: template,
                global_table: tis_ref.global_table.clone(),
            };

            let template_context = TemplateContext::new(provider);

            let scheduler = tis_ref
                .lua
                .app_data_ref::<mlua_scheduler_ext::Scheduler>()
                .unwrap();

            let event = Event::from_create_event(&event);
            let args = match (event, template_context).into_lua_multi(&tis_ref.lua) {
                Ok(f) => f,
                Err(e) => {
                    // Mark memory error'd VMs as broken automatically to avoid user grief/pain
                    if let LuaError::MemoryError(_) = e {
                        // Mark VM as broken
                        tis_ref
                            .broken
                            .store(true, std::sync::atomic::Ordering::Release);
                    }

                    return LuaVmResult::LuaError { err: e.to_string() };
                }
            };

            let Ok(value) = scheduler.spawn_thread_and_wait("Exec", thread, args).await else {
                return LuaVmResult::LuaError {
                    err: "Failed to spawn thread".to_string(),
                };
            };

            let json_value = if let Some(Ok(values)) = value {
                match values.len() {
                    0 => serde_json::Value::Null,
                    1 => {
                        let value = values.into_iter().next().unwrap();

                        match tis_ref.lua.from_value::<serde_json::Value>(value) {
                            Ok(v) => v,
                            Err(e) => {
                                return LuaVmResult::LuaError { err: e.to_string() };
                            }
                        }
                    }
                    _ => {
                        let mut arr = Vec::with_capacity(values.len());

                        for v in values {
                            match tis_ref.lua.from_value::<serde_json::Value>(v) {
                                Ok(v) => arr.push(v),
                                Err(e) => {
                                    return LuaVmResult::LuaError { err: e.to_string() };
                                }
                            }
                        }

                        serde_json::Value::Array(arr)
                    }
                }
            } else if let Some(Err(e)) = value {
                return LuaVmResult::LuaError { err: e.to_string() };
            } else {
                serde_json::Value::String("No response".to_string())
            };

            LuaVmResult::Ok {
                result_val: json_value,
            }
        }
        LuaVmAction::Stop {} => {
            // Mark VM as broken
            tis_ref
                .broken
                .store(true, std::sync::atomic::Ordering::Release);
            LuaVmResult::Ok {
                result_val: serde_json::Value::Null,
            }
        }
        LuaVmAction::GetMemoryUsage {} => {
            let used = tis_ref.lua.used_memory();
            LuaVmResult::Ok {
                result_val: serde_json::Value::Number(used.into()),
            }
        }
        LuaVmAction::SetMemoryLimit { limit } => match tis_ref.lua.set_memory_limit(limit) {
            Ok(limit) => LuaVmResult::Ok {
                result_val: serde_json::Value::Number(limit.into()),
            },
            Err(e) => LuaVmResult::LuaError { err: e.to_string() },
        },
    }
}
