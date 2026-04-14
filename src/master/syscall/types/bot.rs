use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// A shard connection (for bot statistics)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardConn {
    /// The status of the shard connection
    pub status: String,
    /// The real latency of the shard connection in milliseconds
    pub latency: f64,
}

/// Status of all shards and other info like total guilds/users
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BotStatus {
    /// A map of shard group ID to shard connection information
    pub shard_conns: HashMap<u32, ShardConn>,
    /// The total number of guilds the bot is connected to
    pub total_guilds: u64,
    /// The total number of users
    pub total_users: u64,
}
