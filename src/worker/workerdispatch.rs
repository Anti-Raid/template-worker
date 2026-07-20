use std::borrow::Cow;

use dapi::UserId;
use khronos_runtime::primitives::LUA_SERIALIZE_OPTIONS;
use khronos_runtime::rt::mlua::prelude::*;
use khronos_runtime::{utils::khronos_value::KhronosValue};
use rand::distr::{Alphanumeric, SampleString};
use serde::{Deserialize, Serialize};
use crate::geese::state::{StateDbFlags, StateOp};
use crate::{geese::tenantstate::DEFAULT_EVENTS, worker::{workerstate::WorkerState, workertenantstate::WorkerTenantState}};

use super::workervmmanager::{Id, WorkerVmManager};
use khronos_runtime::rt::mlua;

/// A WorkerDispatch manages the dispatching of events to a Luau VM
/// with some utility methods
#[derive(Clone)]
pub struct WorkerDispatch {
    /// VM Manager for the worker
    pub vm_manager: WorkerVmManager,
    /// Worker tenant state
    pub tenant_state: WorkerTenantState,
    /// The state all VMs in the WorkerVmManager share
    pub worker_state: WorkerState,
}

impl WorkerDispatch {
    const ERR_SCOPE: &str = "#err";

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
        self.dispatch_event_complex(id, &name, author, data).await
    }

    pub async fn dispatch_event_complex<Data: IntoLua>(&self, id: Id, name: &str, author: Option<UserId>, data: Data) -> LuaResult<KhronosValue> {
        let tenant_state = self.tenant_state.get_cached_tenant_state_for(id)
            .map_err(|e| mlua::Error::external(format!("Failed to get tenant state for ID {id:?}: {e}")))?;

        if !tenant_state.events.contains_key(name) && !DEFAULT_EVENTS.contains(&name) {
            // Event not registered for this tenant, skip
            return Ok(KhronosValue::Null(()));
        }

        let vm_data = self.vm_manager.get_vm_for(id, &self.worker_state, &self.tenant_state)
            .map_err(|e| mlua::Error::external(format!("Failed to get VM for ID {id:?}: {e}")))?;

        if vm_data.runtime.is_broken() {
            return Err(mlua::Error::external("Lua VM to dispatch to is broken"));
        }

        match vm_data.runtime.call_in_scheduler::<_, KhronosValue>(vm_data.dispatch_func, Event { name, author, data }).await {
            Ok(result) => Ok(result),
            Err(e) => {
                let err_str = e.to_string();
                if let Err(e) = self.save_error(id, err_str).await {
                    log::error!("Failed to log error for ID {id:?}: {}", e);
                }
                Err(e)
            },
        }
    }

    /// Saves a error directly to key-value API
    async fn save_error(&self, id: Id, error: String) -> Result<(), crate::Error> {
        let key = Alphanumeric.sample_string(&mut rand::rng(), 64);
        let ops = vec![StateOp::KvSet { key, scope: Self::ERR_SCOPE.into(), value: KhronosValue::Text(error.into()), blob: None }];
        let res = self.worker_state.mesophyll_client.exec_state_op(id, ops, StateDbFlags::WORKER_INITIATED).await?;
        if let Some(ref ts) = res.new_tenant_state {
            self.tenant_state.reload_for_tenant(id, ts)?;
        }
        Ok(())
    }
}

pub struct Event<'a, Data: IntoLua> {
    name: &'a str,
    author: Option<UserId>,
    data: Data,
}

impl<'a, Data: IntoLua> IntoLua for Event<'a, Data> {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let tab = lua.create_table()?;
        tab.set("name", self.name)?;
        match self.author {
            Some(author) => tab.set("author", author.to_string())?,
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
    FeedTicketRequest(Vec<String>)
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
            Self::FeedTicketRequest(topics) => {
                let tab = lua.create_table_with_capacity(0, 2)?;
                tab.set("topics", topics)?;
                tab.set_readonly(true);
                Ok(LuaValue::Table(tab))
            }
        }
    }
}

/// An `SimpleEvent` is a/an thread-safe object that can be used to create a Event
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SimpleEvent {
    /// The name of the event
    name: Cow<'static, str>,
    /// The author of the event
    author: Option<UserId>,
    /// The inner data of the object
    data: SimpleEventData,
}

impl SimpleEvent {
    /// Create a new Event given a khronos value
    pub fn new_khronos_value(name: String, author: Option<UserId>, data: KhronosValue) -> Self {
        Self { name: name.into(), author, data: SimpleEventData::KhronosValue(data) }
    }

    /// Create a new Event given a raw json string
    pub fn new_json_string(name: String, author: Option<UserId>, data: String) -> Self {
        Self { name: name.into(), author, data: SimpleEventData::JsonString(data) }
    }

    /// Create a new Event for a feed ticket request
    pub fn new_feed_ticket_request(author: Option<UserId>, topics: Vec<String>) -> Self {
        Self { name: "FeedTicketRequest".into(), author, data: SimpleEventData::FeedTicketRequest(topics) }
    }
}