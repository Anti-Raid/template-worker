use crate::objectstore::ObjectStore;
use std::fmt::Debug;
use std::sync::Arc;

/// This struct stores base/standard command data, which is stored and accessible in all command invocations
#[derive(Clone)]
pub struct Data {
    pub pool: sqlx::PgPool,
    pub reqwest: reqwest::Client,
    pub object_store: Arc<ObjectStore>,
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
