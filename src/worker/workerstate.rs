use std::sync::Arc;
use crate::{geese::{objectstore::ObjectStore, stratum::Stratum}, mesophyll::client::MesophyllClient};


#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct WorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub object_store: Arc<ObjectStore>,
    pub current_user: Arc<serenity::all::CurrentUser>,
    pub mesophyll_client: Arc<MesophyllClient>,
    pub stratum: Stratum,
    pub worker_print: bool,
}

impl WorkerState {
    /// Creates a new CreateWorkerState with the given serenity context, reqwest client, object store, and database pool
    pub fn new(
        serenity_http: Arc<serenity::http::Http>,
        object_store: Arc<ObjectStore>,
        current_user: Arc<serenity::all::CurrentUser>,
        mesophyll_client: Arc<MesophyllClient>,
        stratum: Stratum,
        worker_print: bool
    ) -> Self {
        Self {
            serenity_http,
            object_store,
            current_user,
            mesophyll_client,
            stratum,
            worker_print
        }
    }
}
