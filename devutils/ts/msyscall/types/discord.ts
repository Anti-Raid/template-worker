export interface DashboardGuild {
  id: string;
  name: string;
  icon?: string | null;
  permissions: string;
  owner: boolean;
}

export interface DashboardGuildData {
  guilds: DashboardGuild[];
  guilds_exist: boolean[];
}

export interface GuildChannelWithPermissions {
  /** User permissions (Serialized as a stringified bitfield) */
  user: string;
  /** Bot permissions (Serialized as a stringified bitfield) */
  bot: string;
  /** Channel data */
  channel: ApiPartialGuildChannel;
}

export interface ApiPartialGuildChannel {
  /** The ID of the channel */
  id: string;
  /** The name of the channel */
  name: string;
  /** The position of the channel in the guild */
  position: number;
  /** The ID of the parent channel, if any */
  parent_id?: string | null;
  /** The type of the channel */
  type: number;
}

export interface ApiPartialRole {
  /** The ID of the role */
  id: string;
  /** The name of the role */
  name: string;
  /** The position of the role in the guild */
  position: number;
  /** Permissions of the role (Serialized as a stringified bitfield) */
  permissions: string;
}

export interface BaseGuildUserInfo {
  owner_id: string;
  name: string;
  icon?: string | null;
  /** List of all roles in the server */
  roles: ApiPartialRole[];
  /** List of roles the user has */
  user_roles: string[];
  /** List of roles the bot has */
  bot_roles: string[];
  /** List of all channels in the server */
  channels: GuildChannelWithPermissions[];
}

/** * The PartialUser of a user, which contains only the necessary fields for the API 
 */
export interface PartialUser {
  /** The ID of the user */
  id: string;
  /** The username of the user */
  username: string;
  /** The global name of the user */
  global_name?: string | null;
  /** The avatar hash of the user */
  avatar?: string | null;
}