local Primitives = require"@antiraid-core/primitives"
local types = require"../backups"
local load = require"./load"
local apitypes = require"@discord-types/apiTypes"
local resttypes = require"@discord-types/restTypes"
local blob = require"@antiraid-core/blob"
local discordpermcalc = require"@antiraid-ext/utils/discordpermcalc"
local permission = require"@discord-types/permission" 
local RestoreManager = require"./impl"
local sb = require"@antiraid-ext/utils/statusbuffer"

export type PermData = {
    currentBotMember: apitypes.GuildMemberObject,
    currentGuild: apitypes.GuildObject,
}

export type WebhookData = {
    webhookid: string,
    webhooktoken: string,
}

export type GuildFlags = {
    community: boolean,
    animated_icon: boolean,
    banner: boolean,
    animated_banner: boolean,
    invite_splash: boolean
}

--- Partial restore manager
export type PartialRestoreManager = {
    --- Returns the base restore manager
    getBaseManager: () -> RestoreManager.BaseRestoreManager,
    --- Restores messages to a channel from the backup using a webhook
    restoreChannelMessages: (tgtChannel: apitypes.ChannelObject, messages: {apitypes.MessageObject}, webhook: WebhookData, permData: PermData) -> (),
}

--- Sets up a partial restore manager for restoring bits and pieces of a backup
local function PartialRestoreManager(ctx: Primitives.TemplateContext, src: types.RestoreSource, enckey: string?, updater: sb.StatusBuffer, opts: types.BackupRestoreOpts): PartialRestoreManager
    local basemanager = RestoreManager(ctx, src, enckey, updater, {opts = opts})
    local loadedBackup = basemanager.getLoadedBackup()
    local coreData = loadedBackup.core

    local self = {}

    local function getBaseManager() 
        return basemanager
    end

    -- Restore the base guild settings from the backup
    local function restoreBaseGuildSettings() 
        updater.addStatus("Restoring guild data...")

        -- Note: verification_level and explicit_content_filter are not restored for now
        -- and owner, region can't be restored
        local desc = coreData.guild.description
        if desc and #desc > 300 then
            desc = desc:sub(1, 300)
            updater.addStatus("Guild description is longer than 300 characters, truncating to 300 characters")
        end
        local updateData: resttypes.ModifyGuildRequest = {
            name = coreData.guild.name,
            description = desc,
            default_message_notifications = coreData.guild.default_message_notifications,
            system_channel_flags = coreData.guild.system_channel_flags,
            afk_timeout = coreData.guild.afk_timeout,
        }

        local icon = loadedBackup.assets["icon"] :: blob.Blob?
        if icon then
            updateData.icon = load.assetToDataURL("image/jpeg", icon)
        end

        local banner = loadedBackup.assets["banner"] :: blob.Blob?
        if banner then
            updateData.banner = load.assetToDataURL("image/jpeg", banner)
        end

        local splash = loadedBackup.assets["splash"] :: blob.Blob?
        if splash then
            updateData.splash = load.assetToDataURL("image/jpeg", splash)
        end

        ctx.discord:modify_guild({
            reason = "Restore backup: Restore base guild settings step",
            data = updateData
        })

        updater.addStatus("Done modifying basic server settings")
    end

    --- Restores the messages of a channel from the backup
    local function restoreChannelMessages(
        tgtChannel: apitypes.ChannelObject,
        messages: {apitypes.MessageObject},
        webhook: WebhookData,
        permData: PermData
    ) 
        -- Check if we can restore messages to this channel
        local perms = discordpermcalc.MemberChannelPerms(permData.currentGuild, permData.currentBotMember, tgtChannel)
        local canManageWebhooks = discordpermcalc.HasPermission(perms, permission.PermissionSet.ManageWebhooks)
        if not canManageWebhooks then 
            updater.addStatus("Bot does not have 'Manage Webhooks' permissions in channel ``" .. tgtChannel.name .. "`` with ID: ``" .. tgtChannel.id .. "`` , ignoring message restore...")
            return
        else 
            updater.addStatus("Restoring " .. #messages .. " messages to channel ``" .. tgtChannel.name .. "`` with ID: ``" .. tgtChannel.id .. "``")
        end

        --- TODO: This is currently waiting on Webhook support in Khronos
        error("Not yet implemented")
    end

    self.getBaseManager = getBaseManager
    self.restoreBaseGuildSettings = restoreBaseGuildSettings
    self.restoreChannelMessages = restoreChannelMessages

    return self
end

return PartialRestoreManager