use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};
use crate::{events::StartupEvent, worker::{keyexpirychannel::KeyExpiryChannel, workerfilter::WorkerFilter, workerstate::WorkerState}};
use super::workervmmanager::{Id, WorkerVmManager, VmData};
use super::vmcontext::TemplateContextProvider;
use crate::events::AntiraidEvent;
use crate::dispatch::parse_event;
use khronos_runtime::rt::mlua;

/// A WorkerDispatch manages the dispatching of events to a Luau VM
/// with some utility methods
#[derive(Clone)]
pub struct WorkerDispatch {
    /// VM Manager for the worker
    vm_manager: WorkerVmManager,
    /// Worker filter
    filter: WorkerFilter,
    /// Key expiry channel
    key_expiry_chan: KeyExpiryChannel,
}

impl WorkerDispatch {
    /// Creates a new WorkerDispatch with the given WorkerVmManager
    pub fn new(vm_manager: WorkerVmManager, key_expiry_chan: KeyExpiryChannel, filter: WorkerFilter) -> Self {
        let dispatch = Self { vm_manager, key_expiry_chan, filter };

        // Fire resume keys on creation
        let self_ref = dispatch.clone();
        tokio::task::spawn_local(async move {
            if let Err(e) = self_ref.dispatch_startup_events().await {
                log::error!("Failed to dispatch startup events on WorkerDispatch creation: {}", e);
            }
        });

        dispatch
    }

    /// Returns the underlying WorkerState
    pub fn worker_state(&self) -> &WorkerState {
        self.vm_manager.worker_state()
    }

    /// Dispatches startup events for all tenants
    pub async fn dispatch_startup_events(&self) -> Result<(), crate::Error> {
        let ids = self.vm_manager.worker_state().get_startup_event_tenants()?;
        for id in ids.iter() {
            let id = *id;
            if !self.filter.is_allowed(id) {
                continue;
            }
            
            log::info!(
                "Dispatching startup event for ID {id:?}",
            );

            let event = AntiraidEvent::OnStartup(StartupEvent {
                reason: "Worker startup".to_string(),
            });

            let tevent = parse_event(&event)?;

            let self_ref = self.clone();
            tokio::task::spawn_local(async move {
                if let Err(e) = self_ref.dispatch_event(id, tevent).await {
                    log::error!("Failed to dispatch startup event for ID {id:?}: {e}");
                }
            });
        }
        Ok(())
    }

    /// Dispatches an event to the appropriate VM based on the tenant ID
    pub async fn dispatch_event(&self, id: Id, event: CreateEvent) -> mlua::Result<KhronosValue> {
        use khronos_runtime::rt::mlua;

        let tenant_state = self.vm_manager.worker_state().get_cached_tenant_state_for(id)
            .map_err(|e| mlua::Error::external(format!("Failed to get tenant state for ID {id:?}: {e}")))?;

        if !tenant_state.events.contains(event.name()) {
            // Event not registered for this tenant, skip
            return Ok(KhronosValue::Null);
        }

        let vm_data = self.vm_manager.get_vm_for(id).await
            .map_err(|e| mlua::Error::external(format!("Failed to get VM for ID {id:?}: {e}")))?;

        if vm_data.runtime.is_broken() {
            return Err(mlua::Error::external("Lua VM to dispatch to is broken"));
        }

        let func: khronos_runtime::rt::mlua::Function = vm_data
        .runtime
        .eval_script("./builtins.templateloop")?;

        let provider = TemplateContextProvider::new(
            id,
            vm_data.clone(),
            self.key_expiry_chan.clone()
        );
        let context = vm_data.runtime.create_context(provider, event)?;
        match vm_data.runtime.call_in_scheduler::<_, KhronosValue>(func, context).await {
            Ok(result) => Ok(result),
            Err(e) => {
                let err_str = e.to_string();
                tokio::task::spawn_local(async move {
                    if let Err(e) = Self::log_error_to_main_server(&vm_data, err_str.clone()).await {
                        log::error!("Failed to log error for ID {id:?}: {}", e);
                    }
                });
                Err(e)
            },
        }
    }

    /// Returns an Discord error message for a template error
    fn error_message(
        error: String,
    ) -> serde_json::Value {
        serde_json::json!({
            "embeds": [
                {
                    "title": "Error executing template",
                    "description": error,
                    "fields": [],
                }
            ],
            "components": [
                {
                    "type": 1,
                    "components": [
                        {
                            "type": 2,
                            "style": 5,
                            "label": "Support Server",
                            "url": crate::CONFIG.meta.support_server_invite.to_string(),
                        },
                    ]
                }
            ],
        })
    }

    /// Helper method to log a template error to the main server
    async fn log_error_to_main_server(
        vm_data: &VmData,
        error: String,
    ) -> Result<(), crate::Error> {
        let error = format!("```lua\n{}```", error.replace('`', "\\`"));
        // Send to main server
        vm_data.state.serenity_http.send_message(
            crate::CONFIG.meta.default_error_channel.widen(),
            Vec::with_capacity(0),
            &Self::error_message(error),
        )
        .await?;

        Ok(())
    }
}