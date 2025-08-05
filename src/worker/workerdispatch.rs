use std::sync::Arc;

use khronos_runtime::{primitives::event::{CreateEvent, Event}, require::FilesystemWrapper, rt::{mlua::Result as LuaResult, IsolateData, KhronosIsolate}, utils::khronos_value::KhronosValue};
use std::time::Duration;
use crate::templatingrt::template::Template;
use super::workervmmanager::{Id, WorkerVmManager, VmData};
use super::limits::MAX_TEMPLATES_RETURN_WAIT_TIME;
use super::vmcontext::TemplateContextProvider;

/// The result from a template execution
type TemplateResult = Result<KhronosValue, crate::Error>;

/// A WorkerDispatch manages the dispatching of events to a Luau VM
pub struct WorkerDispatch {
    vm_manager: WorkerVmManager
}

impl WorkerDispatch {
    /// Creates a new WorkerDispatch with the given WorkerVmManager
    pub fn new(vm_manager: WorkerVmManager) -> Self {
        Self { vm_manager }
    }

    /// Dispatches an event to the appropriate VM based on the tenant ID without waiting for a response
    pub async fn dispatch_event(&self, id: Id, event: CreateEvent, templates: Vec<Arc<Template>>) -> Result<Vec<(String, TemplateResult)>, crate::Error> {
        let vm_data = self.vm_manager.get_vm_for(id).await
            .map_err(|e| format!("Failed to get VM for ID {id:?}: {e}"))?;

        let res = self.dispatch_event_to_templates(templates, event, vm_data, id).await;

        match res {
            Ok(r) => Ok(r),
            Err(e) => {
                self.log_error(id, e.to_string()).await;
                Err(e)
            }
        }
    }

    /// Logs an error
    async fn log_error(&self, id: Id, error: String) {
        // TODO: Implement error logging to Discord
    }

    /// Helper method to dispatch an event to a single template
    async fn dispatch_event_to_template(
        template: Arc<Template>,
        event: Event,
        vm_data: VmData,
        id: Id,
    ) -> Result<KhronosValue, crate::Error> {
        if vm_data.runtime_manager.runtime().is_broken() {
            return Err("Lua VM to dispatch to is broken".into());
        }

        // Get or create a subisolate
        let (sub_isolate, created_context) = if let Some(sub_isolate) =
            vm_data.runtime_manager.get_sub_isolate(&template.name)
        {
            (sub_isolate.isolate, sub_isolate.data)
        } else {
            let mut attempts = 0;
            let sub_isolate = loop {
                // It may take a few attempts to create a subisolate successfully
                // due to ongoing Lua VM operations
                match KhronosIsolate::new_subisolate(
                    vm_data.runtime_manager.runtime().clone(),
                    FilesystemWrapper::new(template.content.0.clone()),
                    false // TODO: Allow safeenv optimization one day
                ) {
                    Ok(isolate) => {
                        break isolate;
                    }
                    Err(e) => {
                        log::error!("WorkerDispatch: Failed to create subisolate: {e}. This is an internal bug that should not happen");
                        attempts += 1;
                        if attempts >= 20 {
                            return Err(format!("Failed to create subisolate: {e}. This is an internal bug that should not happen").into());
                        }

                        // Wait a bit before retrying
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        // Check if the runtime is broken
                        if vm_data.runtime_manager.runtime().is_broken() {
                            return Err("Lua VM to dispatch to is broken".into());
                        }
                    }
                }
            };

            log::info!("Created subisolate for template {}", template.name);

            let provider = TemplateContextProvider::new(vm_data.state, template.clone(), id);

            let created_context = match sub_isolate.create_context(provider) {
                Ok(ctx) => ctx,
                Err(e) => {
                    return Err(format!("Failed to create context for template {}: {}", template.name, e).into());
                }
            };

            let iso_data = IsolateData {
                isolate: sub_isolate.clone(),
                data: created_context.clone(),
            };

            vm_data.runtime_manager.add_sub_isolate(template.name.clone(), iso_data);

            (sub_isolate, created_context)
        };

        let spawn_result = sub_isolate
            .spawn_asset("/init.luau", "/init.luau", created_context, event)
            .await
            .map_err(|e| e.to_string())?;

        let value = match spawn_result.into_khronos_value(&sub_isolate) {
            Ok(v) => v,
            Err(e) => {
                return Err(format!("Failed to convert result to JSON: {}", e).into())
            }
        };

        Ok(value)
    }

    /// Dispatches an event to templates and returns the results
    async fn dispatch_event_to_templates(
        &self,
        templates: Vec<Arc<Template>>,
        event: CreateEvent,
        vm_data: VmData,
        id: Id,
    ) -> Result<Vec<(String, TemplateResult)>, crate::Error> {        
        if vm_data.runtime_manager.runtime().is_broken() {
            return Err("Lua VM to dispatch to is broken".into());
        }

        let num_templates = templates.len();
        log::debug!("Dispatching event to {} templates", num_templates);

        let mut set = tokio::task::JoinSet::new();

        let event = Event::from_create_event_with_runtime(vm_data.runtime_manager.runtime(), event)
            .map_err(|e| format!("Failed to create event: {}", e))?;

        for template in templates {
            let vm_ref = vm_data.clone();
            let event_ref = event.clone();
            set.spawn_local(async move {
                let name = template.name.clone();
                let result = Self::dispatch_event_to_template(template, event_ref, vm_ref, id).await;

                (name, result)
            });
        }

        let mut results = Vec::with_capacity(num_templates);
        while let Ok(Some(result)) =
            tokio::time::timeout(MAX_TEMPLATES_RETURN_WAIT_TIME, set.join_next()).await
        {
            match result {
                Ok((name, result)) => {
                    results.push((name, result));
                }
                Err(e) => {
                    log::error!("Failed to dispatch event to template: {}", e);
                }
            }
        }

        /*let data = vm_data.state.serenity_context.data::<crate::Data>();
        if let Err(e) = regenerate_deferred(&vm_data.state.serenity_context, &data, guild_state.guild_id).await {
            log::error!("Failed to regenerate deferred: {}", e);
        };*/

        Ok(results)
    }
}