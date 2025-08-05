use std::sync::Arc;
use super::limits::{LuaKVConstraints, Ratelimits};

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct WorkerState {
    pub serenity_context: serenity::all::Context,
    pub reqwest_client: reqwest::Client,
    pub object_store: Arc<crate::objectstore::ObjectStore>,
    pub pool: sqlx::PgPool,

    pub kv_constraints: LuaKVConstraints,
    pub ratelimits: Arc<Ratelimits>,
}

impl WorkerState {
    /// Creates a new WorkerState with the given serenity context, reqwest client, object store, and database pool
    pub fn new(
        serenity_context: serenity::all::Context,
        reqwest_client: reqwest::Client,
        object_store: Arc<crate::objectstore::ObjectStore>,
        pool: sqlx::PgPool,
    ) -> Result<Self, crate::Error> {
        Ok(Self {
            serenity_context,
            reqwest_client,
            object_store,
            pool,
            kv_constraints: LuaKVConstraints::default(),
            ratelimits: Arc::new(Ratelimits::new()?),
        })
    }
}