use std::sync::Arc;

use khronos_runtime::primitives::LUA_SERIALIZE_OPTIONS;
use khronos_runtime::rt::mlua::prelude::*;
use khronos_runtime::{utils::khronos_value::KhronosValue};
use serde::{Deserialize, Serialize};
use crate::{geese::tenantstate::DEFAULT_EVENTS, worker::{workerstate::WorkerState, workertenantstate::WorkerTenantState}};

use super::workervmmanager::{Id, WorkerVmManager};
use khronos_runtime::rt::mlua;

/// A WorkerDispatch manages the dispatching of events to a Luau VM
/// with some utility methods
#[derive(Clone)]
pub struct WorkerDispatch {
    /// VM Manager for the worker
    vm_manager: WorkerVmManager,
    /// Worker tenant state
    tenant_state: WorkerTenantState,
    /// The state all VMs in the WorkerVmManager share
    worker_state: WorkerState,
}

impl WorkerDispatch {
    /// Creates a new WorkerDispatch with the given WorkerVmManager
    pub fn new(vm_manager: WorkerVmManager, worker_state: WorkerState, tenant_state: WorkerTenantState) -> Self {
        let dispatch = Self { vm_manager, worker_state, tenant_state };

        // Dispatch startup events for all tenants in the background upon creation of the WorkerDispatch
        dispatch.dispatch_startup_events();

        dispatch
    }

    /// Dispatches startup events for all tenants
    pub fn dispatch_startup_events(&self) {
        let ids = self.tenant_state.get_startup_event_tenants();
        for id in ids.iter() {
            let id = *id;
            log::info!(
                "Dispatching startup event for ID {id:?}",
            );

            let self_ref = self.clone();
            tokio::task::spawn_local(async move {
                if let Err(e) = self_ref.dispatch_event_complex(id, "OnStartup", None, OnStartupData { reason: "worker_startup"}).await {
                    log::error!("Failed to dispatch startup event for ID {id:?}: {e}");
                }
            });
        }
    }

    /// Dispatches an event to the appropriate VM based on the tenant ID
    pub async fn dispatch_event(&self, id: Id, event: SimpleEvent) -> LuaResult<KhronosValue> {
        let (name, author, data) = (event.name, event.author, event.data);
        self.dispatch_event_complex(id, &name, author.as_deref(), data).await
    }

    pub async fn dispatch_event_complex<Data: IntoLua>(&self, id: Id, name: &str, author: Option<&str>, data: Data) -> LuaResult<KhronosValue> {
        let tenant_state = self.tenant_state.get_cached_tenant_state_for(id)
            .map_err(|e| mlua::Error::external(format!("Failed to get tenant state for ID {id:?}: {e}")))?;

        if !tenant_state.events.contains_key(name) && !DEFAULT_EVENTS.contains(&name) {
            // Event not registered for this tenant, skip
            return Ok(KhronosValue::Null);
        }

        let vm_data = self.vm_manager.get_vm_for(id, &self.worker_state, &self.tenant_state)
            .map_err(|e| mlua::Error::external(format!("Failed to get VM for ID {id:?}: {e}")))?;

        if vm_data.runtime.is_broken() {
            return Err(mlua::Error::external("Lua VM to dispatch to is broken"));
        }

        let http = self.worker_state.serenity_http.clone();
        match vm_data.runtime.call_in_scheduler::<_, KhronosValue>(vm_data.dispatch_func, Event { name, author, data }).await {
            Ok(result) => Ok(result),
            Err(e) => {
                let err_str = e.to_string();
                tokio::task::spawn_local(async move {
                    if let Err(e) = Self::log_error_to_main_server(http, err_str.clone()).await {
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
        serenity_http: Arc<serenity::all::Http>,
        error: String,
    ) -> Result<(), crate::Error> {
        let error = format!("```lua\n{}```", error.replace('`', "\\`"));
        // Send to main server
        serenity_http.send_message(
            crate::CONFIG.meta.default_error_channel.widen(),
            Vec::with_capacity(0),
            &Self::error_message(error),
        )
        .await?;

        Ok(())
    }
}

pub struct Event<'a, Data: IntoLua> {
    name: &'a str,
    author: Option<&'a str>,
    data: Data
}

impl<'a, Data: IntoLua> IntoLua for Event<'a, Data> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let tab = lua.create_table()?;
        tab.set("name", self.name)?;
        match self.author {
            Some(author) => tab.set("author", author)?,
            None => {},
        }
        tab.set(
            "data",
            self.data
        )?;
        tab.set_readonly(true);
        Ok(LuaValue::Table(tab))
    }
}

pub struct OnStartupData {
    reason: &'static str
}

impl IntoLua for OnStartupData {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let tab = lua.create_table_with_capacity(0, 1)?;
        tab.set("reason", self.reason)?;
        tab.set_readonly(true);
        Ok(LuaValue::Table(tab))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Inner event data to a simple event
enum SimpleEventData {
    KhronosValue(KhronosValue),
    JsonString(String),
}

impl IntoLua for SimpleEventData {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            Self::KhronosValue(value) => {
                value.into_lua(lua)
            },
            Self::JsonString(ref value) => {
                let value: serde_json::Value = serde_json::from_str(value)
                    .map_err(|e| LuaError::external(e))?;
                lua.to_value_with(&value, LUA_SERIALIZE_OPTIONS)
            },
        }
    }
}

/// An `SimpleEvent` is a/an thread-safe object that can be used to create a Event
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SimpleEvent {
    /// The name of the event
    name: String,
    /// The author of the event
    author: Option<String>,
    /// The inner data of the object
    data: SimpleEventData,
}

impl SimpleEvent {
    /// Create a new Event given a khronos value
    pub fn new_khronos_value(name: String, author: Option<String>, data: KhronosValue) -> Self {
        Self { name, author, data: SimpleEventData::KhronosValue(data) }
    }

    /// Create a new Event given a raw json string
    pub fn new_json_string(name: String, author: Option<String>, data: String) -> Self {
        Self { name, author, data: SimpleEventData::JsonString(data) }
    }
}