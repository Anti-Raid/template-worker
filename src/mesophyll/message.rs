use khronos_runtime::{primitives::event::CreateEvent, utils::khronos_value::KhronosValue};

use crate::{mesophyll::dbtypes::CreateGlobalKv, worker::workervmmanager::Id};

#[derive(serde::Serialize, serde::Deserialize)]
pub enum ServerMessage {
    /// Acknowledgment that the worker is identified and session is active
    Hello { heartbeat_interval_ms: u64 },

    /// Dispatch a (custom) template event to a worker
    /// 
    /// Discord related events are dispatched via Sandwich
    DispatchEvent { 
        id: Id, 
        event: CreateEvent, 
        req_id: Option<u64> 
    },

    /// Run a script in the worker
    RunScript { 
        id: Id, 
        name: String, 
        code: String, 
        event: CreateEvent,
        req_id: u64 
    },

    /// Requests that a worker be dropped
    DropWorker { 
        id: Id, 
        req_id: u64 
    },
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum ClientMessage {
    /// Response to a event dispatch with the result of a dispatched event
    DispatchResponse { 
        req_id: u64, 
        result: Result<KhronosValue, String> 
    },

    /// A heartbeat message to keep the connection alive
    Heartbeat { },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum KeyValueOp {
    Get { 
        scopes: Vec<String>,
        key: String 
    },
    ListScopes {},
    Set { 
        scopes: Vec<String>,
        key: String, 
        value: KhronosValue 
    },
    Delete { 
        scopes: Vec<String>,
        key: String 
    },
    Find { 
        scopes: Vec<String>,
        prefix: String
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum PublicGlobalKeyValueOp {
    Find {
        query: String,
        scope: String
    },
    Get { 
        key: String,
        version: i32,
        scope: String,
        id: Option<Id>,
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum GlobalKeyValueOp {
    Create {
        entry: CreateGlobalKv
    },
    Delete {
        key: String,
        version: i32,
        scope: String,
    },
}