local channel = require"@discord-types/channel"
local types = require"../backups"
local discordpermcalc = require"@antiraid-ext/utils/discordpermcalc"
local permission = require"@discord-types/permission"
local RestoreManager = require"./impl"
local CheckpointManager = require"@antiraid-ext/checkpoint"
local datetime = require"@antiraid/datetime"

local CHECKPOINT_EXPIRY = datetime.timedelta_minutes(15)

export type Opts = {
    opts: types.BackupRestoreOpts,
    src: types.RestoreSource, 
    enckey: string?, 
    gre: RestoreManager.GetRestored,
}

--- Helper function to get the restore manager from the slate or create a new one if it doesn't exist. This is used to persist the restore manager across checkpoints.
local function getmanagerfromslate(ctx: CheckpointManager.CheckpointContext<Opts>): RestoreManager.BaseRestoreManager
    local slate = ctx.slate
    if slate.rm then 
        return slate.rm
    end

    local data = ctx.data()
    local rm = RestoreManager(ctx.tctx, data.src, data.enckey, ctx.sb, {
        opts = data.opts,
        gre = data.gre
    })

    local loadedBackup = rm.getLoadedBackup()
    local coreData = loadedBackup.core
    local guildData = rm.getCurrentGuildData()
    rm.validateBackupCompatibility()
    assert(coreData.guild.id, "internal error: guild ID must be set in core data")

    -- Run the basic permission checks for restoring a backup before doing any more intensive operations, to fail fast if the bot doesn't have the required permissions
    if not discordpermcalc.HasPermission(guildData.basePerms, permission.PermissionSet.ManageChannels) then
        error("Cannot restore backup: bot does not have the 'Manage Channels' permission")
    end

    if not discordpermcalc.HasPermission(guildData.basePerms, permission.PermissionSet.ManageRoles) then
        error("Cannot restore backup: bot does not have the 'Manage Roles' permission")
    end

    if coreData.options.backupMessages and not discordpermcalc.HasPermission(guildData.basePerms, permission.PermissionSet.ManageWebhooks) then
        error("Cannot restore backup: bot does not have the 'Manage Webhooks' permission but backup contains messages")
    end

    slate.rm = rm
    return rm
end

-- The steps on how to restore a backup
local RESTORE_STEPS: {CheckpointManager.CheckpointStep<Opts>} = {
    {
        step = "EditBaseGuild",
        fn = function(ctx) 
            local basemanager = getmanagerfromslate(ctx)
            basemanager.restoreBaseGuildSettings()
            return nil
        end
    },
    {
        step = "DeleteOldRoles",
        fn = function(ctx) 
            local basemanager = getmanagerfromslate(ctx)
            local guildData = basemanager.getCurrentGuildData()

            assert(guildData.currentGuild.roles, "Internal Error: guildData.currentGuild.roles should not be nil here")
            basemanager.deleteRoles(guildData.currentGuild.roles)
            return nil
        end
    },
    {
        step = "CreateRoles",
        fn = function(ctx) 
            local basemanager = getmanagerfromslate(ctx)
            local data = ctx.data()
            local coreData = basemanager.getLoadedBackup().core

            assert(coreData.guild.roles, "Internal Error: coreData.guild.roles should not be nil here")

            basemanager.createRoles(coreData.guild.roles, function(_role) 
                -- Update checkpoint
                data.gre = basemanager.getRestored() 
                ctx.save(data)
            end)

            -- Update currentGuild roles to the restored roles
            basemanager.syncCurrentGuild()

            data.gre = basemanager.getRestored() 
            return data
        end
    },
    {
        step = "DeleteOldChannels",
        fn = function(ctx) 
            local basemanager = getmanagerfromslate(ctx)
            local guildData = basemanager.getCurrentGuildData()

            basemanager.deleteChannels(guildData.currentChannels)
            return nil -- preserve old checkpoint data since we will need it for the next step
        end
    },
    {
        step = "CreateChannels",
        fn = function(ctx) 
            local basemanager = getmanagerfromslate(ctx)
            local data = ctx.data()
            local coreData = basemanager.getLoadedBackup().core
            assert(coreData.channels, "Internal Error: coreData.channels should not be nil here")

            local chans = {}
            
            -- First categories
            for _, c in coreData.channels do 
                if c.type ~= channel.ChannelTypesMap.GuildCategory then 
                    continue
                end

                table.insert(chans, c)
            end
            -- Then channels
            for _, c in coreData.channels do 
                if c.type == channel.ChannelTypesMap.GuildCategory then 
                    continue
                end

                table.insert(chans, c)
            end

            -- Restore
            basemanager.createChannels(chans, function(_chan) 
                -- Update checkpoint
                data.gre = basemanager.getRestored() 
                ctx.save(data)
            end)

            data.gre = basemanager.getRestored()
            return data
        end
    },
    {
        step = "FinishBaseRestore",
        fn = function(ctx) 
            local basemanager = getmanagerfromslate(ctx)
            basemanager.finishBaseRestore()
            return nil
        end
    }
}

return {
    RESTORE_STEPS = RESTORE_STEPS,
    CHECKPOINT_EXPIRY = CHECKPOINT_EXPIRY
}