use super::{resolve_template_to_bytecode, ArLuaThreadInnerState, LuaVmAction, LuaVmResult};
use mlua::prelude::*;
use mlua_scheduler_ext::traits::{IntoLuaThread, LuaSchedulerExt};

/// Handles a Lua VM action, returning a result
pub async fn handle_event(action: LuaVmAction, tis_ref: &ArLuaThreadInnerState) -> LuaVmResult {
    match action {
        LuaVmAction::Exec {
            content,
            template,
            pragma,
            event,
        } => {
            if tis_ref.broken.load(std::sync::atomic::Ordering::Acquire) {
                return LuaVmResult::VmBroken {};
            }

            let exec_name = match template {
                crate::Template::Raw(_) => "script".to_string(),
                crate::Template::Named(ref name) => name.to_string(),
            };

            // Check bytecode cache first, compile template if not found
            let template_bytecode = match resolve_template_to_bytecode(
                content,
                template.clone(),
                &tis_ref.bytecode_cache,
            )
            .await
            {
                Ok(bytecode) => bytecode,
                Err(e) => {
                    return LuaVmResult::LuaError {
                        err: e.to_string(),
                        template_name: Some(exec_name),
                    };
                }
            };

            // Now, create the template context that should be passed to the template
            let template_context = super::ctx::TemplateContext::new(super::state::TemplateData {
                path: match template {
                    crate::Template::Raw(_) => "".to_string(),
                    crate::Template::Named(ref name) => name.clone(),
                },
                template,
                pragma,
            });

            let thread = match tis_ref
                .lua
                .load(&template_bytecode)
                .set_name(&exec_name)
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

                    return LuaVmResult::LuaError {
                        err: e.to_string(),
                        template_name: Some(exec_name),
                    };
                }
            };

            // Mark thread with template name
            let thread_tracker = tis_ref
                .lua
                .app_data_ref::<mlua_scheduler_ext::feedbacks::ThreadTracker>()
                .unwrap();

            thread_tracker.set_metadata(thread.clone(), exec_name.clone());

            let th = match tis_ref
                .lua
                .push_thread_back(thread, (event, template_context))
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

                    return LuaVmResult::LuaError {
                        err: e.to_string(),
                        template_name: Some(exec_name),
                    };
                }
            };

            let thread_error_tracker = tis_ref
                .lua
                .app_data_ref::<crate::lang_lua::ThreadErrorTracker>()
                .unwrap();

            let value = thread_error_tracker
                .wait_for_result_timeout(th, crate::MAX_TEMPLATES_RETURN_WAIT_TIME)
                .await;

            let json_value = if let Some(values) = value {
                if values.len() > 1 {
                    let mut arr = Vec::with_capacity(values.len());

                    for v in values {
                        match tis_ref.lua.from_value::<serde_json::Value>(v) {
                            Ok(v) => arr.push(v),
                            Err(e) => {
                                return LuaVmResult::LuaError {
                                    err: e.to_string(),
                                    template_name: Some(exec_name),
                                };
                            }
                        }
                    }

                    serde_json::Value::Array(arr)
                } else {
                    let value = values.into_iter().next().unwrap();

                    match tis_ref.lua.from_value::<serde_json::Value>(value) {
                        Ok(v) => v,
                        Err(e) => {
                            return LuaVmResult::LuaError {
                                err: e.to_string(),
                                template_name: Some(exec_name),
                            };
                        }
                    }
                }
            } else {
                serde_json::Value::Null
            };

            // Send acknoledgement
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
            Err(e) => LuaVmResult::LuaError {
                err: e.to_string(),
                template_name: None,
            },
        },
    }
}
