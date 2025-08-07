use std::sync::Arc;

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct WorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub reqwest_client: reqwest::Client,
    pub object_store: Arc<crate::objectstore::ObjectStore>,
    pub pool: sqlx::PgPool,
    pub current_user: Arc<serenity::all::CurrentUser>,
}

impl WorkerState {
    /// Creates a new WorkerState with the given serenity context, reqwest client, object store, and database pool
    pub fn new(
        serenity_http: Arc<serenity::http::Http>,
        reqwest_client: reqwest::Client,
        object_store: Arc<crate::objectstore::ObjectStore>,
        pool: sqlx::PgPool,
        current_user: Arc<serenity::all::CurrentUser>,
    ) -> Result<Self, crate::Error> {
        Ok(Self {
            serenity_http,
            reqwest_client,
            object_store,
            pool,
            current_user
        })
    }
}

// Assert that WorkerThread is Send + Sync + Clone
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync + Clone>() {}
    assert_send_sync_clone::<WorkerState>();
};
