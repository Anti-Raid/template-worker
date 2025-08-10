// TODO

/// A WorkerProcessPool stores a pool of worker processes in which servers are evenly distributed via
/// the Discord Id sharding formula:
/// 
/// shard_id = (guild_id >> 22) % num_shards
/// 
/// Each worker process has a single WorkerThread internally on the process and handles its own dispatches via tokio and sandwich layer with only settings events
/// and API related events being sent via IPC from master process to worker process
#[allow(dead_code)]
pub struct WorkerProcessPool {
    // The processes in the pool
    //processes: Vec<WorkerProcess>,
}
