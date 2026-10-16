export interface GuildLockdown { id: string; type?: string | null; reason?: string | null; created_at: string }

export interface GuildBackup {
  id: string;
  created_by?: string | null; size?: number | null;
  created_at: string;
  last_updated_at: string;
}
