local mode = require("../mode")
local handle = require("../handle")
local utils = require("../utils")
local discord = require "@discord-types/apiTypes"
local set = require"@antiraid-ext/set"

type RoleLockdownData = {
    global_perms: string,
    channel_overrides: {[discord.Snowflake]: discord.OverwriteObject}
}

local function RoleLockdown(roleid: discord.Snowflake): mode.LockdownMode 
    local self = {}

    local function _fromdata(data: any): string
        if type(data) ~= "string" then
            error("Invalid data format for RoleLockdown")
        end

        return data
    end

    local function string_form(): string
        return "role/" .. roleid
    end

    --- ROLE > TSL as it updates a single channel
    local function specificity(): number
        return 2
    end

    --- ROLE doesn't need to test anything so this is a no-op
    local function test_noop(_data: mode.BaseLockdownModeData): string?
        return nil
    end

    local function autofix_noop(_data: mode.BaseLockdownModeData): ()
        error("A Role Lockdown does not support autofixing")
    end

    local function setup(data: mode.BaseLockdownModeData) 
        assert(data.guild.roles, "Internal error: Guild roles not found")
        local globalperms: string? = nil
        for _, role in data.guild.roles do 
            if role.id == roleid then
                globalperms = role.permissions
                break
            end
        end

        assert(globalperms, "Role not found while setting up RoleLockdown")

        return globalperms
    end

    local function shareable(lockdown: mode.Lockdown): handle.LockdownSharableData 
        local data = _fromdata(lockdown.data)

        return {
            role_permissions = { [roleid] = data },
            channel_permissions = {}
        }
    end

    local function create(data: mode.ExtLockdownModeData) 
        if data.all_handles.isrolelocked(roleid, specificity()) then 
            return -- Someone else is handling this role
        end

        -- Edit the role
        local newrole = data.ctx.discord:modify_guild_role({
            reason = "Role lockdown",
            role_id = roleid,
            data = {
                permissions = "0" -- Clear all permissions
            }
        }).data

        utils.replacerole(data, newrole)
    end

    local function revert(data: mode.ExtLockdownModeData) 
        if data.all_handles.isrolelocked(roleid, specificity()) then 
            return -- Someone else is handling this channel
        end
        
        local oldpermissions = _fromdata(data.lockdown.data)

        -- Edit the role back
        local newrole = data.ctx.discord:modify_guild_role({
            reason = "Role lockdown",
            role_id = roleid,
            data = {
                permissions = oldpermissions
            }
        }).data

        utils.replacerole(data, newrole)
    end

    local function handle(data: mode.BaseLockdownModeData, lockdown: mode.Lockdown): handle.LockdownModeHandle
        -- Role just locks the single role its operating on
        local roles = set.Set() :: set.Set<discord.Snowflake>
        roles.add(roleid)
        return {
            roles = roles,
            channels = set.Set() :: set.Set<discord.Snowflake>,
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

return RoleLockdown 