use std::time::Duration;

/// Mesophyll provides a coordination layer between all the different workers along
/// with the master process. It is the (WIP) replacement for WorkerProcessComm and uses
/// WebSockets instead of HTTP2 for communication. This enables for stuff like the template 
/// shop where an update on one worker may need to be dispatched/broadcasted to other workers.
/// 
/// Mesophyll is currently implemented/runs on the master process itself enabling the
/// master process to store the full state and dispatch it out to workers when needed.
///
/// In the future, it is a goal for Mesophyll to be a base unit of sandboxing as well
/// through projects like khronos dapi
pub mod client;
pub mod server;

pub const STREAM_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct StreamId {
    pub id: String,
    pub worker_id: usize
}

impl StreamId {
    fn extract_worker_id(stream_id: &str) -> Option<usize> {
        let bits = stream_id.split_once(';');
        if let Some((top, _)) = bits {
            top.parse().ok()
        } else {
            None
        }
    }

    pub fn new(id: String) -> Option<Self> {
        let wid = Self::extract_worker_id(&id)?;
        Some(Self { id, worker_id: wid })
    }
}
