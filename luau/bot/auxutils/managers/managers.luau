local stingmanager = require"../stingmanager"
local afkmanager = require"../afkmanager"
local honeypotmanager = require"../honeypotmanager"
local Primitives = require"@antiraid-core/primitives"
local KeyExpiryManager = require "@antiraid-ext/keyexpirymanager"
local UncachedKeyManager = require "@antiraid-ext/uncachedkeymanager"
local CheckpointManager = require "@antiraid-ext/checkpoint"
local LockdownSet = require"../lockdowns/lockdownset"
local AuditLogManager = require"../auditlogmanager"
local scriptmanager = require"../scriptmanager"
local load = require"../backups/restore/load"
local restoresteps = require"../backups/restore/restore"

export type TempBan = {
    userid: string,
    modid: string?,
    reason: string,
}

export type RemindMe = {
    userid: string,
    channelid: string,
    message: string,
    ephemeral: boolean,
}

export type BackupMetadata = load.BackupMetadata

export type Managers = {
    stingmanager: stingmanager.StingManager,
    afkmanager: afkmanager.AFKManager,
    honeypotmanager: honeypotmanager.HoneypotManager,
    tempbankex: KeyExpiryManager.KeyExpiryManager<TempBan>,
    remindmeexpiry: KeyExpiryManager.KeyExpiryManager<RemindMe>,
    --afkexpiry: KeyExpiryManager.KeyExpiryManager<afkmanager.AFKExpiryData>,
    --stingexpiry: KeyExpiryManager.KeyExpiryManager<stingmanager.StingExpiryData>,
    backupcheckpointmanager: CheckpointManager.CheckpointManager<restoresteps.Opts>,
    lockdownset: LockdownSet.LockdownSet,
    auditlogmanager: AuditLogManager.AuditLogManager,
    scriptmanager: scriptmanager.ScriptManager,
    backupmetadata: UncachedKeyManager.UncachedKeyManager<BackupMetadata>
}

local managers: Managers? = nil

--- Gets or initializes the built-in managers
local function getmanagers(ctx: Primitives.TemplateContext): Managers
    print("Got managers", {a=1})
    if managers then return managers end
    
    local managersref: Managers = {} :: Managers

    local tempbanExpiryManager = KeyExpiryManager<<TempBan>>(ctx, "builtins.tempbans", function(record) 
        -- userid = userid, modid = modid, reason = reason, [createdat = createdat, expiresat = expiresat]
        local userid = record.value.userid
        local modid = record.value.modid
        local reason = record.value.reason
        if not userid or not reason then 
            return
        end
        -- Unban the user
        local ok, res = xpcall(function()
            ctx.discord:remove_guild_ban({user_id = userid, reason = "Temporary ban expired: "..reason})
            return nil
        end, debug.traceback :: any)
        -- Dispatch an event with the result of the unban attempt so other templates can react to it if needed
        ctx.loop.dispatch{
            name = "BuiltinsTempBanExpiry", 
            data = { 
                userid = userid, 
                modid = modid, 
                reason = reason, 
                createdat = record.createdat, 
                expiresat = record.expiresat,
                ok = ok, 
                res = res
            }, 
            author = nil
        }
        return nil
    end)
    local remindmeExpiryManager = KeyExpiryManager<<RemindMe>>(ctx, "builtins.remindme", function(record) 
        local userid = record.value.userid
        local channelid = record.value.channelid
        local message = record.value.message
        local ephemeral = record.value.ephemeral
        if not userid or not channelid or not message then 
            return
        end

        -- Send the reminder message
        ctx.discord:create_message({
            channel_id = channelid,
            data = {
                embeds = {
                    {
                        title = "Reminder",
                        description = string.format("<@%s> %s", userid, message),
                    }
                }
            }
        })

        ctx.loop.dispatch{
            name = "BuiltinsRemindMeExpiry", 
            data = { 
                userid = userid,
                channelid = channelid,
                message = message,
                ephemeral = ephemeral,
                createdat = record.createdat,
                expiresat = record.expiresat,
            }, 
            author = nil
        }
        return nil
    end)

    local afkmanager = afkmanager(ctx)
    local stingmanager = stingmanager(ctx)
    local honeypotmanager = honeypotmanager(ctx)
    local backupcheckpointmanager = CheckpointManager<<restoresteps.Opts>>(ctx, "builtins.backups", restoresteps.CHECKPOINT_EXPIRY, restoresteps.RESTORE_STEPS)
    local lockdownset = LockdownSet(ctx)
    local auditlogmanager = AuditLogManager.AuditLogManager(ctx)
    local scriptmanager = scriptmanager.TemplateManager(ctx, {managers = managersref})

    managersref.stingmanager = stingmanager
    managersref.afkmanager = afkmanager
    managersref.honeypotmanager = honeypotmanager
    managersref.tempbankex = tempbanExpiryManager
    managersref.remindmeexpiry = remindmeExpiryManager
    --managersref.afkexpiry = afkExpiryManager
    --managersref.stingexpiry = stingExpiryManager
    managersref.backupcheckpointmanager = backupcheckpointmanager
    managersref.lockdownset = lockdownset
    managersref.auditlogmanager = auditlogmanager
    managersref.scriptmanager = scriptmanager
    managersref.backupmetadata = load.getbackupmetadata(ctx)

    managers = managersref

    return managers or error("Failed to initialize managers")
end

return {
    getmanagers = getmanagers,
}