use std::sync::Arc;

use khronos_runtime::{primitives::event::CreateEvent};

use crate::{templatedb::{attached_templates::TemplateOwner, template_shop_listing::TemplateShopListing}, worker::workerprocesscomm::WorkerProcessCommDispatchResult};

/// A message that is relayed to all connected Mesophyll clients
#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub enum MesophyllRelayMessage {
    /// Shop template has been updated
    ShopTemplateUpdate { listing: Arc<TemplateShopListing> },
}

/// The messages Mesophyll can send to a worker.
#[derive(serde::Serialize, serde::Deserialize)]
pub enum MesophyllMessage {
    /// Worker is trying to identify itself to Mesophyll
    Identify { id: usize, session_key: String },

    /// Ready message from worker to Mesophyll
    Ready { },

    /// Relay message to all connected clients
    Relay { msg: MesophyllRelayMessage, req_id: u64 },

    // dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> DispatchTemplateResult;
    /// Dispatch an template event to a worker
    /// 
    /// Worker must respond with a DispatchTemplateResult
    DispatchEvent { id: TemplateOwner, event: CreateEvent, req_id: u64 },

    /// Dispatch a scoped template event to a worker
    ///
    /// Worker must respond with a DispatchTemplateResult
    DispatchScopedEvent { id: TemplateOwner, event: CreateEvent, scopes: Vec<String>, req_id: u64 },

    /// Response from worker with the result of a dispatched event
    ResponseDispatchResult { result: WorkerProcessCommDispatchResult, req_id: u64 },

    /// Response that a request has been processed without a result
    ResponseAck { req_id: u64 },

    /// Response that a request has failed with an error
    ResponseError { error: String, req_id: u64 },
}
