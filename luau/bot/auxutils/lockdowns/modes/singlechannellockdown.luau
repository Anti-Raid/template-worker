local permissions = require"@discord-types/permission"
local mode = require("../mode")
local handle = require("../handle")
local utils = require("../utils")
local discord = require "@discord-types/apiTypes"
local set = require"@antiraid-ext/set"

--- The perms to deny on overwrite roles
local DENY_PERMS: {permissions.Permissions} = {
    "SendMessages",
    "SendMessagesInThreads",
    "SendTTSMessages",
    "Connect",
}

local function SingleChannelLockdown(channelid: discord.Snowflake): mode.LockdownMode 
    local self = {}

    local function _fromdata(data: any): {discord.OverwriteObject}
        if type(data) ~= "table" then
            error("Invalid data format for SingleChannelLockdown")
        end

        for _, overwrite in data do
            if type(overwrite) ~= "table" then 
                error("Invalid overwrite format for SingleChannelLockdown [not a array of overwrites]")
            end
            overwrite = overwrite :: discord.OverwriteObject
            if not overwrite.id or not overwrite.type then
                error("Invalid overwrite format for SingleChannelLockdown")
            end
        end

        return data
    end

    local function string_form(): string
        return "scl/" .. channelid
    end

    --- SCL > TSL as it updates a single channel
    local function specificity(): number
        return 2
    end

    --- SCL doesn't need to test anything so this is a no-op
    local function test_noop(_data: mode.BaseLockdownModeData): string? 
        return nil
    end

    local function autofix_noop(_data: mode.BaseLockdownModeData): ()
        error("A Single Channel Lockdown does not support autofixing")
    end

    local function setup(data: mode.BaseLockdownModeData) 
        local map: {discord.OverwriteObject}? = nil

        -- Get the channel overwrites for the specific channel
        for _, channel in data.channels do 
            if channel.id ~= channelid then continue end -- Only interested in the specific channel
            local overwrites = channel.permission_overwrites

            -- Check for an underlying permission to the role
            local underlying = utils.getunderlyingchannelpermissions(data.lockdowns, channel.id)
            if underlying then overwrites = underlying end -- Update if underlying is present

            map = overwrites or {} -- Use the overwrites or an empty table if none exist
        end

        assert(map ~= nil, "Channel not found while setting up SingleChannelLockdown")

        return map
    end

    local function shareable(lockdown: mode.Lockdown): handle.LockdownSharableData 
        local data = _fromdata(lockdown.data)
        return {
            role_permissions = {},
            channel_permissions = { [channelid] = data }
        }
    end

    local function create(data: mode.ExtLockdownModeData) 
        if data.all_handles.ischannellocked(channelid, specificity()) then 
            return -- Someone else is handling this channel
        end

        local overwrites = _fromdata(data.lockdown.data) -- We just set this up via setup() earlier
        
        local pendingOverwriteRoles = data.critical_roles.copy()

        for _, overwrite in overwrites do 
            -- Ignore non-role overwrites
            if overwrite.type ~= permissions.OverwriteObjectTypeRole then continue end
            if data.critical_roles.exists(overwrite.id) then 
                pendingOverwriteRoles.remove(overwrite.id) -- Remove from pending if already exists
                overwrite.deny = tostring(permissions.toBitFlag(DENY_PERMS))
                overwrite.allow = tostring(permissions.toBitFlag({})) -- Clear the allow perms
            end
        end

        -- Add new overwrites for critical roles
        for roleId in pendingOverwriteRoles do
            local newOverwrite: discord.OverwriteObject = {
                id = roleId,
                type = permissions.OverwriteObjectTypeRole,
                deny = tostring(permissions.toBitFlag(DENY_PERMS)),
                allow = tostring(permissions.toBitFlag({})),
            }
            table.insert(overwrites, newOverwrite)
        end

        local newchannel = data.ctx.discord:edit_channel({
            reason = "Single channel lockdown",
            channel_id = channelid,
            data = {
                permission_overwrites = overwrites,
            }
        }).data

        utils.replacechannel(data, newchannel)
    end

    local function revert(data: mode.ExtLockdownModeData) 
        if data.all_handles.ischannellocked(channelid, specificity()) then 
            return -- Someone else is handling this channel
        end
        
        local oldpermissions = _fromdata(data.lockdown.data)

        local newchannel = data.ctx.discord:edit_channel({
            reason = "Single channel lockdown revert",
            channel_id = channelid,
            data = {
                permission_overwrites = oldpermissions,
            }
        }).data

        utils.replacechannel(data, newchannel)
    end

    local function handle(data: mode.BaseLockdownModeData, lockdown: mode.Lockdown): handle.LockdownModeHandle
        -- SCL just locks the single channel its operating on
        local channels = set.Set() :: set.Set<discord.Snowflake>
        channels.add(channelid)
        return {
            roles = set.Set() :: set.Set<discord.Snowflake>,
            channels = channels,
        }
    end

    self.string_form = string_form
    self.specificity = specificity
    self.test = test_noop
    self.autofix = autofix_noop
    self.setup = setup
    self.shareable = shareable
    self.create = create
    self.revert = revert
    self.handle = handle

    return self
end

return SingleChannelLockdown 