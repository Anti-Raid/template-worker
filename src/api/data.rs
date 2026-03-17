use crate::geese::stratum::Stratum;
use crate::worker::workerlike::WorkerLike;
use std::fmt::Debug;
use std::sync::Arc;

/// This struct stores base/standard data to be used anywhere in template-worker
#[derive(Clone)]
pub struct ApiData {
    pub current_user: serenity::all::CurrentUser,
    pub reqwest: reqwest::Client,
    pub worker: Arc<dyn WorkerLike + Send + Sync>,
    pub stratum: Stratum
}

impl Debug for ApiData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Data")
            .field("reqwest", &"reqwest::Client")
            .finish()
    }
}
