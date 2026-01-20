use crate::objectstore::ObjectStore;
use crate::sandwich::Sandwich;
use crate::worker::workerlike::WorkerLike;
use std::fmt::Debug;
use std::sync::Arc;

/// This struct stores base/standard data to be used anywhere in template-worker
#[derive(Clone)]
pub struct Data {
    pub current_user: serenity::all::CurrentUser,
    pub reqwest: reqwest::Client,
    #[allow(dead_code)]
    pub object_store: Arc<ObjectStore>,
    pub worker: Arc<dyn WorkerLike + Send + Sync>,
    pub sandwich: Sandwich,
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("reqwest", &"reqwest::Client")
            .field("object_store", &"Arc<ObjectStore>")
            .finish()
    }
}
