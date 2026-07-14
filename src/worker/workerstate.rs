use std::sync::Arc;
use crate::{geese::stratum::Stratum, mesophyll::client::MesophyllClient};


#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct WorkerState {
    pub mesophyll_client: Arc<MesophyllClient>,
    pub stratum: Stratum,
    pub worker_print: bool,
    pub reqwest: reqwest::Client,
}

impl WorkerState {
    /// Creates a new CreateWorkerState with the given serenity context, reqwest client, object store, and database pool
    pub fn new(
        mesophyll_client: Arc<MesophyllClient>,
        stratum: Stratum,
        reqwest: reqwest::Client,
        worker_print: bool
    ) -> Self {
        Self {
            mesophyll_client,
            stratum,
            reqwest,
            worker_print
        }
    }
}
