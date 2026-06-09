import type { DashboardGuildData, BaseGuildUserInfo } from '../types/discord'

export type MDiscordSyscall = 
  | { 
      /** Get a list of all user guilds from Discord */
      op: "GetUserGuilds"; 
      /** Whether to force a refresh of the guild list from Discord API */
      refresh: boolean 
    }
  | { 
      /** Get detailed information about a specific guild */
      op: "GetGuildInfo"; 
      /** The ID of the guild to fetch information for */
      guild_id: string 
    };

export type MDiscordSyscallRet = 
  | { 
      /** List of all user guilds response */
      op: "UserGuilds"; 
      /** The user's guild data including existence on the bot */
      data: DashboardGuildData 
    }
  | { 
      /** Guild information response */
      op: "GuildInfo"; 
      /** Detailed guild, member, and channel information */
      data: BaseGuildUserInfo 
    };
