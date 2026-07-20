local permissions = require"@discord-types/permission"
local mode = require("../mode")
local handle = require("../handle")
local utils = require("../utils")
local discord = require "@discord-types/apiTypes"
local set = require"@antiraid-ext/set"

-- The base permissions for quick lockdown
--
-- If any of these permissions are provided, quick lockdown cannot proceed
local BASE_PERMS: {permissions.Permissions} = {
    "ViewChannel",
    "SendMessages",
}

-- Stores the internal set of changes required for the quick server lockdown to perform as expected
type RoleChange = {
    op: "Add" | "Remove",
    perms: {permissions.Permissions},
    role: discord.GuildRoleObject
}

local LOCKDOWN_PERMS: {permissions.Permissions} = {"ViewChannel"}

local function FullServerLockdown(): mode.LockdownMode 
    local self = {}

    local function _fromdata(data: any): {[discord.Snowflake]: string}
        for roleId, perm in data do
            if type(roleId) ~= "string" or type(perm) ~= "string" then
                error("Invalid data format for FullServerLockdown")
            end
        end
        return data
    end

    local function string_form(): string
        return "full"
    end

    -- Lowest specificity, as it is a quick lockdown
    local function specificity(): number
        return 0
    end

    -- Internal method to collect needed role changes
    local function _test(data: mode.BaseLockdownModeData): {RoleChange}
        assert(data.guild.roles, "Internal error: Guild roles not found")

        local changes: {RoleChange} = {}

        for _, role in data.guild.roles do 
            if data.critical_roles.exists(role.id) then
                local rolePerms = permissions.fromBitFlagToSet(role.permissions)

                local neededPerms = {}
                for _, perm in BASE_PERMS do
                    if not rolePerms.exists(perm) then
                        table.insert(neededPerms, perm)
                    end
                end

                if #neededPerms > 0 then
                    table.insert(changes, {
                        op = "Add" :: "Add",
                        perms = neededPerms,
                        role = role
                    })
                end
            else 
                local rolePerms = permissions.fromBitFlagToSet(role.permissions)

                local neededPerms = {}
                for _, perm in BASE_PERMS do
                    if rolePerms.exists(perm) then
                        table.insert(neededPerms, perm)
                    end
                end

                if #neededPerms > 0 then
                    table.insert(changes, {
                        op = "Remove" :: "Remove",
                        perms = neededPerms,
                        role = role
                    })
                end
            end
        end

        -- TODO: In the future, check channels too

        return changes
    end

    local function test(data: mode.BaseLockdownModeData): string? 
        local changes = _test(data)
        if #changes > 0 then
            local errMsg = "The following changes are required for the quick server lockdown to proceed:\n"
            for _, change in changes do
                errMsg = errMsg .. string.format("- %s %s %s (%s)\n", change.op, table.concat(change.perms, ", "), change.role.name, change.role.id)
            end
            return errMsg
        end
        return nil
    end

    local function autofix(data: mode.BaseLockdownModeData) 
        local changeset = _test(data)
        for _, change in changeset do 
            local newperms = change.role.permissions
            if change.op == "Add" then 
                newperms = permissions.addToPerms(change.role.permissions, change.perms)
            else 
                newperms = permissions.removeFromPerms(change.role.permissions, change.perms)
            end
            
            local role = data.ctx.discord:modify_guild_role({ reason = "Autofix attempted", role_id = change.role.id, data = { permissions = tostring(newperms) } }).data

            utils.replacerole(data, role)
        end
    end

    local function setup(data: mode.BaseLockdownModeData) 
        assert(data.guild.roles, "Internal error: Guild roles not found")

        local map: {[discord.Snowflake]: string} = {}

        -- Collect map of roles with the underlying perms
        for _, role in data.guild.roles do 
            local perms = role.permissions

            -- Check for an underlying permission to the role
            local underlying = utils.getunderlyingrolepermissions(data.lockdowns, role.id)
            if underlying then perms = underlying end -- Update if underlying is present

            map[role.id] = perms
        end

        return map
    end

    local function shareable(lockdown: mode.Lockdown): handle.LockdownSharableData 
        local data = _fromdata(lockdown.data)
        return {
            role_permissions = data,
            channel_permissions = {}
        }
    end

    local function create(data: mode.ExtLockdownModeData) 
        assert(data.guild.roles, "Internal error: Guild roles not found")
        for idx, role in data.guild.roles do 
            if data.critical_roles.exists(role.id) then
                local newrole = data.ctx.discord:modify_guild_role({
                    reason = "Quick server lockdown",
                    role_id = role.id,
                    data = {
                        permissions = tostring(permissions.toBitFlag(LOCKDOWN_PERMS))
                    }
                }).data
                utils.replacerole(data, newrole, idx)            
            end
        end
    end

    local function revert(data: mode.ExtLockdownModeData) 
        assert(data.guild.roles, "Internal error: Guild roles not found")

        local oldpermissions = _fromdata(data.lockdown.data)
        for idx, role in data.guild.roles do 
            if data.critical_roles.exists(role.id) then
                local perms = tostring(oldpermissions[role.id] or permissions.toBitFlag(BASE_PERMS))

                local newrole = data.ctx.discord:modify_guild_role({
                    reason = "Quick server lockdown",
                    role_id = role.id,
                    data = {
                        permissions = perms
                    }
                }).data
                utils.replacerole(data, newrole, idx)            
            end
        end
    end

    local function handle(data: mode.BaseLockdownModeData, lockdown: mode.Lockdown): handle.LockdownModeHandle
        -- FullServerLockdown locks the critical roles
        return {
            roles = data.critical_roles,
            channels = set.Set() :: set.Set<discord.Snowflake>,
        }
    end

    self.string_form = string_form
    self.specificity = specificity
    self.test = test
    self.autofix = autofix
    self.setup = setup
    self.shareable = shareable
    self.create = create
    self.revert = revert
    self.handle = handle

    return self
end

return FullServerLockdown