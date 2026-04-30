export interface ShardConn {
  /** The status of the shard connection */
  status: string;
  /** The real latency of the shard connection in milliseconds */
  latency: number;
}

export interface BotStatus {
  /** A map of shard group ID to shard connection information */
  shard_conns: Record<number, ShardConn>;
  /** The total number of guilds the bot is connected to */
  total_guilds: number;
  /** The total number of users */
  total_users: number;
  /** The current uptime of the bot process in seconds */
  uptime: number;
}

export interface BotConfig {
  /** The ID of the main AntiRaid support server */
  main_server: string;
  /** Discord Support Server Link */
  support_server_invite: string;
  /** The ID of the AntiRaid bot client */
  client_id: string;
}
