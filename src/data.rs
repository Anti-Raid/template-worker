use crate::objectstore::ObjectStore;
use crate::worker::workerlike::WorkerLike;
use std::fmt::Debug;
use std::sync::Arc;

/// This struct stores base/standard command data, which is stored and accessible in all command invocations
#[derive(Clone)]
pub struct Data {
    pub current_user: serenity::all::CurrentUser,
    pub pool: sqlx::PgPool,
    pub reqwest: reqwest::Client,
    #[allow(dead_code)]
    pub object_store: Arc<ObjectStore>,
    pub worker: Arc<dyn WorkerLike + Send + Sync>,
}

impl Debug for Data {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("pool", &"sqlx::PgPool")
            .field("reqwest", &"reqwest::Client")
            .field("object_store", &"Arc<ObjectStore>")
            .finish()
    }
}
