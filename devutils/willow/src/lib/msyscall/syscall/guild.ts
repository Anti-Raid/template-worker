import { type GuildBackup, type GuildLockdown } from '../types/guild'

export type MGuildSyscall = { op: "ListLockdowns"; guild_id: string } | { op: "ListBackups"; guild_id: string };

export type MGuildSyscallRet =
  | { op: "Lockdowns"; lockdowns: GuildLockdown[] }
  | { op: "Backups"; backups: GuildBackup[] };
