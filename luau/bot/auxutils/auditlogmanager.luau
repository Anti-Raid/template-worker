--!strict

local Primitives = require "@antiraid-core/primitives"
local datetime = require "@antiraid/datetime"
local KeyManager = require "@antiraid-ext/keymanager"

local MAX_CHANNELS = 7
local MAX_WHATS_PER_CHANNEL = 5

export type Action = {
    name: string,
    value: string,
    description: string,
}

local actions: {Action} = {
    { name = "All", value = "*", description = "All moderation logs" },
    { name = "Sting Create", value = "sting_create", description = "The log channel for when a sting is created" },
    { name = "Sting Delete", value = "sting_delete", description = "The log channel for when a sting is deleted" },
    { name = "Sting Expiration Set", value = "sting_set_expiration", description = "The long channel for when a stings expiration is manually set"},
    { name = "(Antiraid) Ban", value = "antiraid_ban", description = "The log channel for when a user is banned through the built-in ban command" },
    { name = "(Antiraid) Unban", value = "antiraid_unban", description = "The log channel for when a user is unbanned through the built-in unban command" },
    { name = "(Antiraid) Kick", value = "antiraid_kick", description = "The log channel for when a user is kicked through the built-in kick command" },
}

export type What = "*"
    | "sting_create"
    | "sting_delete"
    | "sting_set_expiration"
    | "antiraid_ban"
    | "antiraid_unban"
    | "antiraid_kick"

export type CreateLog = {
    channel_id: string, -- the id of the channel to send logs to
    whats: {What}, -- what action to log
}

export type Log = {
    channel_id: string, -- id of the channel to send logs to
    whats: {What}, -- What action
    created_at: datetime.DateTime, -- When the log was created
}

export type AuditLogManager = {
    --- Returns a list of all audit logs
    list: () -> {Log},
    --- Returns a log by id if it exists, otherwise nil
    get: (channelid: string) -> Log?,
    --- Creates or updates a log entry
    set: (data: CreateLog) -> (),
    --- Deletes an audit log
    delete: (channelid: string) -> (),
}

type LogData = {
    whats: {What},
}

--- An audit log manager for destructive actions
local function AuditLogManager(ctx: Primitives.TemplateContext): AuditLogManager
    local self = {}

    local km = KeyManager<<LogData>>(ctx, "builtins.auditlogs")
    local cachedlist: {Log}? = nil

    local function _parseLog(item: KeyManager.KeyRecord<LogData>): Log
        return {
            channel_id = item.key,
            whats = item.value.whats,
            created_at = item.createdat,
        }
    end

    local function list(): {Log}
        if cachedlist then return cachedlist end
        local data = km.list()

        local logs = {}
        for _, item in data do
            local log = _parseLog(item)
            table.insert(logs, log)
        end

        logs = table.freeze(logs) :: {Log}
        cachedlist = logs
        return logs
    end

    local function get(channelid: string): Log?
        local item = km.get(channelid)
        if not item then return nil end
        return _parseLog(item)
    end

    local function set(logData: CreateLog): ()
        if #logData.whats == 0 or #logData.whats > MAX_WHATS_PER_CHANNEL then
            error("The number of actions to log must be between 1 and 5")
        end
        if km.exists(logData.channel_id) then 
            km.updatedata(logData.channel_id, {
                whats = logData.whats,
            })
        else
            if km.count() >= MAX_CHANNELS then
                error("Maximum number of log channels reached")
            end
            km.add({
                whats = logData.whats,
            }, logData.channel_id)
        end
        cachedlist = nil -- Invalidate cache
    end

    local function delete(channelid: string)
        km.remove(channelid)
        cachedlist = nil -- Invalidate cache
    end

    self.list = list
    self.get = get
    self.set = set
    self.delete = delete

    return self
end

return {
    AuditLogManager = AuditLogManager,
    actions = actions,
}