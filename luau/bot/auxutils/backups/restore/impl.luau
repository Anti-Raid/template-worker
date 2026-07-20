local Primitives = require"@antiraid-core/primitives"
local types = require"../backups"
local load = require"./load"
local apitypes = require"@discord-types/apiTypes"
local resttypes = require"@discord-types/restTypes"
local blob = require"@antiraid-core/blob"
local discordpermcalc = require"@antiraid-ext/utils/discordpermcalc"
local compare = require"@antiraid-ext/utils/rolecmp"
local permission = require"@discord-types/permission"
local channel = require"@discord-types/channel"
local sb = require"@antiraid-ext/utils/statusbuffer"
local conditions = require"../conditions"

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

export type GetRestored = {
    restoredRoles: {[string]: apitypes.GuildRoleObject}, -- current restore map of the old role ID to the new role object (for roles that have already been restored in the restore process, used for fixing permission overwrites)
    restoredChannels: {[string]: apitypes.ChannelObject} -- current restore map of the old channel ID to the new channel object (for channels that have already been restored in the restore process, used for fixing permission overwrites and category assignments)
}

--- Partial restore manager
export type BaseRestoreManager = {
    --- Returns the loaded backup
    getLoadedBackup: () -> types.LoadedBackup,
    --- Gets the current restored roles/channels maps (used for permission overwrite fixing and category assignment fixing during channel restore)
    getRestored: () -> GetRestored,
    --- Returns the current guild data, including current guild object and flags
    ---
    --- # Guarantees
    --- 1. Returned currentGuild.roles will always be sorted in hierarchy order (highest role first)
    --- 2. currentGuild will always have the roles field populated (either from the initial guild object or from an additional API call if it was not included in the initial get_guild response)
    --- 3. currentBotMember will always be included and valid
    getCurrentGuildData: () -> IGuildData,
    --- Validates the backup's compatibility with the current guild and returns the current guild data
    ---
    --- For example, if the backup was taken from a community guild but the current guild is not a community guild, then this function will throw an error as the backup is not compatible with the current guild. 
    ---
    --- This is to prevent users from accidentally restoring incompatible backups which can lead to errors during restores/partial restores
    validateBackupCompatibility: () -> (),
    --- Creates all roles passed calling oncreaterole for each created role
    createRoles: (roles: {apitypes.GuildRoleObject}, oncreaterole: (apitypes.GuildRoleObject) -> ()) -> (),
    --- Deletes roles from the guild. 
    ---
    --- Ignores deleting protected roles or roles that are higher or equal to the bot's highest role
    deleteRoles: (role: {apitypes.GuildRoleObject}) -> (),
    --- Returns if a role is protected from being deleted
    isRoleProtected: (role: apitypes.GuildRoleObject, delete: boolean?) -> boolean,
    --- Returns if a channel is protected from being deleted
    isChannelProtected: (chan: apitypes.ChannelObject, delete: boolean?) -> boolean,
    --- Creates all channels passed calling oncreatechannel with the given data
    createChannels: (channels: {apitypes.ChannelObject}, oncreatechannel: (apitypes.ChannelObject) -> ()) -> (),
    --- Deletes a channel
    ---
    --- Ignores channels the bot does not have 'Manage Channels' permission in or protected channels
    deleteChannels: (channels: {apitypes.ChannelObject}) -> (),
    --- Restores the base guild settings from the backup
    restoreBaseGuildSettings: () -> (),
    --- Finishes the base restore by performing final steps such as syncing the current guild data and deleting old rules/public updates channels if they are not protected
    finishBaseRestore: () -> (),

    --- Updates the current guild data's field with the currently restored roles + other data
    syncCurrentGuild: () -> (),
}

type IGuildData = {
    currentGuild: apitypes.GuildObject,
    currentGuildFlags: GuildFlags,
    currentBotMember: apitypes.GuildMemberObject,
    basePerms: integer,
    botHighestRole: apitypes.GuildRoleObject,
    currentChannels: {apitypes.ChannelObject},
}

type BaseRestoreManagerOpts = {
    opts: types.BackupRestoreOpts,
    gre: GetRestored?,
}

--- Sets up a partial restore manager for restoring bits and pieces of a backup
local function BaseRestoreManager(
    ctx: Primitives.TemplateContext, 
    src: types.RestoreSource, 
    enckey: string?, 
    updater: sb.StatusBuffer, 
    opts: BaseRestoreManagerOpts
): BaseRestoreManager
    local loadedBackup = load.loadBackupFile(ctx, src, enckey, updater)
    local coreData = loadedBackup.core
    assert(coreData.guild.roles, "invalid backup: guild roles data is missing in core data")
    compare.SortRolesByHierarchyDesc(coreData.guild.roles) -- Sort backup roles in hierarchy order
    
    conditions.validate(coreData.options)

    -- Set src and target guild ids and do basic validation
    local srcGuildId = coreData.guild.id
    assert(srcGuildId and #srcGuildId > 0, "invalid backup: guild ID is missing or invalid")
    assert(ctx.btd().id.tenant_type == "guild", "backups can only be used within a guild context")
    local targetGuildId = ctx.btd().id.tenant_id
    assert(targetGuildId and #targetGuildId > 0, "invalid context: guild ID is missing or invalid")

    local cachedGuildData: IGuildData? = nil

    -- Extract protected roles and channels from opts along with the currently restored roles/channels maps
    local protectedRoles = opts.opts.protectedRoles or {}
    local protectedChannels = opts.opts.protectedChannels or {}

    -- Stores the currently restored roles and channels for permission overwrite fixing and category assignment fixing during channel restore. 
    -- Maps old role/channel ID to new role/channel object.
    local gre: GetRestored = opts.gre or {
        restoredRoles = {},
        restoredChannels = {},
    }
    local restoredRoles = gre.restoredRoles
    local restoredChannels = gre.restoredChannels
    
    local self = {}

    -- Helper method to get current guild data
    local function getCurrentGuildData() 
        if cachedGuildData then return cachedGuildData end

        -- Test if coreData is valid    
        assert(coreData.guild.id and #coreData.guild.id > 0, "internal error: guild ID must be set in core data")

        local currentGuild = ctx.discord:get_guild().data

        local currentGuildFlags: GuildFlags = {
            community = false,
            banner = false,
            animated_banner = false,
            animated_icon = false,
            invite_splash = false
        }

        -- Collect features
        if currentGuild.features then 
            for _, feature in currentGuild.features do 
                if feature == "COMMUNITY" then currentGuildFlags.community = true end
                if feature == "ANIMATED_ICON" then currentGuildFlags.animated_icon = true end
                if feature == "BANNER" then currentGuildFlags.banner = true end
                if feature == "ANIMATED_BANNER" then currentGuildFlags.animated_banner = true end
                if feature == "INVITE_SPLASH" then currentGuildFlags.invite_splash = true end
            end
        end

        local currentBotMember = ctx.discord:get_guild_member(ctx.btd().bot.id).data
        if not currentBotMember then
            error("Cannot restore backup: bot is not a member of the guild")
        end

        if not currentGuild.roles or #currentGuild.roles == 0 then 
            local roles = ctx.discord:get_guild_roles().data
            local rolesArr = {}
            for _, role in roles do table.insert(rolesArr, role) end
            currentGuild.roles = rolesArr
        end

        assert(currentGuild.roles, "internal error: guild roles must be loaded before restoring backup")

        compare.SortRolesByHierarchyDesc(currentGuild.roles) -- Sort current roles in hierarchy order

        local basePerms = discordpermcalc.BasePermissions(currentGuild, currentBotMember)

        -- Find bots highest role
        local botHighestRole = nil

        local guildRolesMap = {}
        for _, role in currentGuild.roles do
            guildRolesMap[role.id] = role
        end

        for _, memberRoleId in currentBotMember.roles do 
            local role = guildRolesMap[memberRoleId]
            if not role then
                continue -- Skip roles that are not present in guild_roles
            end

            if not botHighestRole or compare.IsRoleAGreaterThanRoleB(role :: apitypes.GuildRoleObject, botHighestRole) then
                botHighestRole = role :: apitypes.GuildRoleObject
            end
        end

        -- Check highest role
        assert(botHighestRole, "Bot does not have any roles in the guild")

        updater.addStatus("Highest role of bot is " .. botHighestRole.name .. " (ID: " .. botHighestRole.id .. ")")

        assert(botHighestRole.position > 0, "internal error: user's highest role must be above the @everyone role in the guild hierarchy")
        
        local currentChannels = ctx.discord:get_guild_channels().data

        local guild: IGuildData = {
            currentGuild = currentGuild,
            currentGuildFlags = currentGuildFlags,
            currentBotMember = currentBotMember,
            basePerms = basePerms,
            botHighestRole = botHighestRole,
            currentChannels = currentChannels,
        }

        cachedGuildData = guild
        return guild
    end

    local function validateBackupCompatibility() 
        local currentGuildData = getCurrentGuildData()

        local guildWasCommunity = coreData.guild.features and table.find(coreData.guild.features, "COMMUNITY" :: "COMMUNITY") ~= nil
        if guildWasCommunity and not currentGuildData.currentGuildFlags.community then
            error("Cannot restore a community guild backup to a non-community guild. Please enable the community features in your guild settings.")
        end
    end

    local function getLoadedBackup()
        return loadedBackup
    end

    local function getRestored()
        return gre
    end

    -- Restore the base guild settings from the backup
    local function restoreBaseGuildSettings() 
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

    local function isRoleProtected(role: apitypes.GuildRoleObject, delete: boolean?)
        local guildData = getCurrentGuildData()
        if role.managed -- Managed role
        or role.id == guildData.currentGuild.id -- @everyone
        or table.find(protectedRoles, role.id) -- Protected role
        or role.position <= 0 -- Invalid role
        or (delete and compare.IsRoleAGreaterThanOrEqualToRoleB(role, guildData.botHighestRole))
        then 
            return true
        end
        return false
    end

    local function isChannelProtected(chan: apitypes.ChannelObject, delete: boolean?)
        local guildData = getCurrentGuildData()

        if delete then
            if chan.id == guildData.currentGuild.rules_channel_id or chan.id == guildData.currentGuild.public_updates_channel_id then
                return true -- Protected due to being a special channel that the bot cannot delete
            end

            local channelPerms = discordpermcalc.ComputeMemberChannelOverwrites(guildData.basePerms, guildData.currentGuild, guildData.currentBotMember, chan)

            if not discordpermcalc.HasPermission(channelPerms, permission.PermissionSet.ManageChannels) then
                return true -- Protected due to lack of permissions
            end
        end

        if table.find(protectedChannels, chan.id) then 
            return true -- Protected due to being in protected channels list
        end
        return false -- Not protected
    end

    local function deleteRoles(roles: {apitypes.GuildRoleObject})
        -- TODO: Use bulk discord api
        for _, role in roles do
            if isRoleProtected(role, true)
            then 
                updater.addStatus("Ignoring role ``" .. role.name .. "`` with ID: ``" .. role.id .. "`` and position " .. tostring(role.position))
                continue
            end

            updater.addStatus("Deleting role ``" .. role.name .. "`` with ID: ``" .. role.id .. "`` and position " .. tostring(role.position))
            ctx.discord:delete_guild_role({
                reason = "Backup restore: Delete roles step",
                role_id = role.id
            })
        end
    end

    local function createRoles(roles: {apitypes.GuildRoleObject}, oncreaterole: (apitypes.GuildRoleObject) -> ())
        -- TODO: Use bulk discord api
        local guildData = getCurrentGuildData()

        for _, role in roles do 
            if restoredRoles[role.id] or isRoleProtected(role) -- Already restored role
            then 
                --updater.addStatus("Ignoring role ``" .. role.name .. "`` with ID: ``" .. role.id .. "`` and position " .. tostring(role.position))
                continue -- Ignore and continue
            end

            -- Ensure role permissions meet our bots permissions
            if not discordpermcalc.HasPermission(guildData.basePerms, integer.fromstring(role.permissions)) then 
                -- If the role permissions are not allowed by the bot, then do not set any role permissions
                --
                -- TODO: Perform granular disabling of permissions we don't have
                role.permissions = "0"
            end

            -- TODO: Support role icons/role emojis later
            local roleUpdate: resttypes.CreateGuildRoleRequest = {
                name = role.name,
                color = role.color,
                hoist = role.hoist,
                permissions = role.permissions,
                mentionable = role.mentionable,
            }

            updater.addStatus("Creating role ``" .. role.name .. "`` with ID: ``" .. role.id .. "`` and position " ..  tostring(role.position))

            local created = ctx.discord:create_guild_role({
                reason = "Backup restore: Create roles step",
                data = roleUpdate,
            }).data

            -- Cache the created role for permission overwrite fixing later
            restoredRoles[role.id] = created

            oncreaterole(created)
        end
    end

    --- Given a channel to restore and other ext data, create a resttypes.CreateGuildChannelRequest
    local function _createChannelData(
        channel: apitypes.ChannelObject, 
        parentid: string?
    ): resttypes.CreateGuildChannelRequest
        -- Fix permission overwrites
        channel.permission_overwrites = channel.permission_overwrites or {}
        local permOverwrites: {apitypes.OverwriteObject} = table.create(#channel.permission_overwrites) :: {apitypes.OverwriteObject}

        for _, overwrite in channel.permission_overwrites do 
            if overwrite.type == permission.OverwriteObjectTypeRole then 
                if overwrite.id == srcGuildId then
                    -- Special case: @everyone role of source guild
                    table.insert(permOverwrites, {
                        type = overwrite.type,
                        allow = overwrite.allow,
                        deny = overwrite.deny,
                        id = targetGuildId
                    })
                elseif restoredRoles[overwrite.id] then 
                    -- Case: Role in source guild restored to target
                    table.insert(permOverwrites, {
                        type = overwrite.type,
                        allow = overwrite.allow,
                        deny = overwrite.deny,
                        id = restoredRoles[overwrite.id].id
                    })
                elseif table.find(protectedRoles, overwrite.id) then
                    table.insert(permOverwrites, overwrite) -- Protected role
                end
            else
                table.insert(permOverwrites, overwrite)
            end
        end

        local parentId = parentid
        if not parentid then
            if channel.parent_id then 
                if table.find(protectedChannels, channel.parent_id) then
                    parentId = channel.parent_id
                elseif restoredChannels[channel.parent_id] then
                    parentId = restoredChannels[channel.parent_id].id
                else 
                    updater.addStatus(`Parent channel could not be found while processing update data for channel \`\`{channel.id}\`\` and name \`\`{channel.name}\`\``)
                end
            end
        end

        return {
            name = channel.name,
            type = channel.type,
            topic = channel.topic,
            user_limit = channel.user_limit,
            rate_limit_per_user = channel.rate_limit_per_user,
            position = channel.position,
            permission_overwrites = if #permOverwrites > 0 then permOverwrites else nil,
            parent_id = parentId,
            nsfw = channel.nsfw,
            default_thread_rate_limit_per_user = channel.default_thread_rate_limit_per_user,
        }
    end

    local function createChannels(chans: {apitypes.ChannelObject}, oncreatechannel: (apitypes.ChannelObject) -> ())
        -- TODO: Use bulk discord api
        for _, chan in chans do 
            if isChannelProtected(chan) or restoredChannels[chan.id] -- Already restored channel
            then 
                --updater.addStatus("Ignoring channel ``" .. chan.name .. "`` with ID: ``" .. chan.id .. "`` as it is protected or already restored")
                continue -- Ignore and continue
            end

            local ud = _createChannelData(chan)

            if chan.type == channel.ChannelTypesMap.GuildCategory then
                updater.addStatus("Creating category ``" .. chan.name .. "`` with ID: ``" .. chan.id .. "``")
            else 
                updater.addStatus("Creating channel ``" .. chan.name .. "`` with ID: ``" .. chan.id .. "``")
            end

            local createdChannel = ctx.discord:create_guild_channel({
                reason = "Backup restore: Create channels step",
                data = ud,
            }).data

            restoredChannels[chan.id] = createdChannel

            oncreatechannel(createdChannel)
        end
    end

    local function deleteChannels(chans: {apitypes.ChannelObject})
        -- TODO: Use bulk discord api
        for _, chan in chans do 
            if isChannelProtected(chan, true) then
                updater.addStatus("Ignoring channel ``" .. chan.name .. "`` with ID: ``" .. chan.id .. "`` as bot does not have 'Manage Channels' permission in it or channel is in the protected channels list")
                continue -- Ignore and continue
            end

            updater.addStatus("Deleting channel ``" .. chan.name .. "`` with ID: ``" .. chan.id .. "``")

            ctx.discord:delete_channel({
                reason = "Backup restore: Delete old channels step",
                channel_id = chan.id,
            })
        end
    end

    local function syncCurrentGuild() 
        local guildData = getCurrentGuildData()
        local newRoles = {} :: {apitypes.GuildRoleObject}
        for _, role in restoredRoles do 
            table.insert(newRoles, role)
        end
        compare.SortRolesByHierarchyDesc(newRoles)
        guildData.currentGuild.roles = newRoles
        guildData.currentBotMember = ctx.discord:get_guild_member(ctx.btd().bot.id).data -- Set current bot member
    end

    local function finishBaseRestore() 
        local guildData = getCurrentGuildData()
        -- Update rules_channel_id and public_updates_channel_id if COMMUNITY
        if guildData.currentGuildFlags.community then 
            local newRulesChannelId = nil
            local newPublicUpdatesChannelId = nil
            if coreData.guild.rules_channel_id then
                newRulesChannelId = restoredChannels[coreData.guild.rules_channel_id]
            end

            if coreData.guild.public_updates_channel_id then
                newPublicUpdatesChannelId = restoredChannels[coreData.guild.public_updates_channel_id]
            end

            if newRulesChannelId or newPublicUpdatesChannelId then
                local updateData: resttypes.ModifyGuildRequest = {}
                if newRulesChannelId then
                    updateData.rules_channel_id = newRulesChannelId.id
                end
                if newPublicUpdatesChannelId then
                    updateData.public_updates_channel_id = newPublicUpdatesChannelId.id
                end

                ctx.discord:modify_guild({
                    reason = "Backup restore: Finish base restore step",
                    data = updateData,
                })

                updater.addStatus("Updated rules and public updates channels")

                if newRulesChannelId then
                    guildData.currentGuild.rules_channel_id = newRulesChannelId.id
                end

                if newPublicUpdatesChannelId then
                    guildData.currentGuild.public_updates_channel_id = newPublicUpdatesChannelId.id
                end
            end
        end
    end

    self.getLoadedBackup = getLoadedBackup
    self.getRestored = getRestored
    self.getCurrentGuildData = getCurrentGuildData
    self.validateBackupCompatibility = validateBackupCompatibility
    self.restoreBaseGuildSettings = restoreBaseGuildSettings
    self.isRoleProtected = isRoleProtected
    self.isChannelProtected = isChannelProtected
    self.createRoles = createRoles
    self.deleteRoles = deleteRoles
    self.createChannels = createChannels
    self.deleteChannels = deleteChannels
    self.syncCurrentGuild = syncCurrentGuild
    self.finishBaseRestore = finishBaseRestore  

    return self
end

return BaseRestoreManager