--[[
The file format for backups v2:

Internally a backup is a TAR file with the .arb2 file extension. Encrypted backups are simply a AES256 encrypted ARB2 with the .arb2e file extension.

TAR File Contents:
- `core.json.gz`: A JSON file containing the cote backup data.
- `assets/{asset_name}.jpg`: A directory containing all assets that are backed up, such as guild icons (and maybe emojis in the future?).

## Core Backup Data Format

The JSON file contains the following fields:
- `guild`: The guild object from Discord (a `discordTypes.GuildObject`)
- `channels`: The channels in the guild, as an array of `discordTypes.ChannelObject` (this is a subset of the channels that were backed up).
- `messages`: An array of messages (`discordTypes.MessageObject`).
- `options`: The options used to create the backup, as defined in `BackupCreateOpts`.
- `channel_allocation`: The final channel allocation for the backup, mapping channel IDs to the number of messages backed up in that channel.

Format Changelog:

arb2/arb2e format: core.json is now gzipped to save space (and is hence renamed to core.json.gz).
]]
local discordTypes = require("@discord-types/apiTypes")
local blob = require"@antiraid-core/blob"
local assets = require"./create/assetfetch"

export type CoreBackupData = {
    guild: discordTypes.GuildObject, -- The guild object from Discord (a `discordTypes.GuildObject`)
    channels: {discordTypes.ChannelObject}, -- The channels in the guild, as an array of `discordTypes.ChannelObject`
    messages: {[string]: {discordTypes.MessageObject}}, -- An array of messages (`discordTypes.MessageObject`)
    options: BackupCreateOpts, -- The options used to create the backup
    channel_allocation: {[string]: number}, -- The final channel allocation for the backup, mapping channel IDs to the number of messages backed up in that channel
}

export type BackupCreateOpts = {
    channels: {string}, -- The channels to backup
    perChannel: number, -- The number of messages per channel
    maxMessages: number, -- The maximum number of messages to backup/restore
    backupMessages: boolean, -- Whether to backup messages or not
    backupGuildAssets: {"icon" | "banner" | "splash"}, -- Whether to backup guild assets (icon, banner, splash, emojis) or not
    specialAllocations: {[string]: number}, -- Specific channel allocation overrides
}

export type BackupRestoreOpts = {
    protectedChannels: {string}, -- Channels to protect from being deleted
    protectedRoles: {string}, -- Roles to protect from being deleted
}

export type RestoreSource = {
    type: "attachment",
    url: string, -- The URL of the attachment
    filename: string, -- The name of the file including the extension
} | {
    type: "kv",
    key: string, -- The key in the kv store where the backup blob is stored
}

export type BuiltBackup = {
    id: string,
    backup: blob.Blob,
    filename: string, -- The name of the backup file including the extension
}

export type LoadedBackup = {
    core: CoreBackupData,
    assets: {[assets.Asset]: blob.Blob}, -- A map of asset names to their blobs
}

return {}