local mode = require("./mode")
local handle = require("./handle")
local permissions = require("@discord-types/permission")
local discord = require("@discord-types/apiTypes")
local set = require"@antiraid-ext/set"

--- Returns the critical roles for a guild
---
--- The ``set_member_roles`` here are the pre-defined roles set by the server that should be locked down
--- If ``set_member_roles`` is empty (no overrides), the @everyone role is returned
local function getCriticalRoles(guild: discord.GuildObject, set_member_roles: {discord.Snowflake}): set.Set<discord.Snowflake>
    if #set_member_roles == 0 then
        -- Find the everyone role
        assert(guild.roles, "Internal error: Guild roles are not set")
        for _, role in guild.roles do
            if role.id == guild.id then
                return set.fromtable({role.id})
            end
        end
        error("Internal error: No @everyone role found")
    else
        return set.fromtable(set_member_roles)
    end
end

--- Merges a set of lockdown sharable data's assuming that least recent lockdowns are first
local function mergelsd(lsd: {handle.LockdownSharableData}): handle.LockdownSharableData     
    local new_channel_perms = {}

    -- Add all new/unique permission overwrites of users/roles
    --
    -- If said users overwrites are already present in the map, new overwrites will be ignored.
    --
    -- This works because the least recent lockdowns are checked first
    local per_channel_done_roles = {}
    local per_channel_done_users = {}

    for _, data in lsd do
        for channel_id, overwrites in data.channel_permissions do
            if not per_channel_done_roles[channel_id] then
                per_channel_done_roles[channel_id] = {}
            end
            if not per_channel_done_users[channel_id] then
                per_channel_done_users[channel_id] = {}
            end
            local done_roles = per_channel_done_roles[channel_id]
            local done_users = per_channel_done_users[channel_id]
            if not new_channel_perms[channel_id] then
                new_channel_perms[channel_id] = {}
            end
            local channel_pos = new_channel_perms[channel_id]

            for _, overwrite in overwrites do
                if overwrite.type == permissions.OverwriteObjectTypeRole then
                    local role_id = overwrite.id
                    if not done_roles[role_id] then
                        table.insert(channel_pos, overwrite)
                        done_roles[role_id] = true
                    end
                elseif overwrite.type == permissions.OverwriteObjectTypeMember then
                    local user_id = overwrite.id
                    if not done_users[user_id] then
                        table.insert(channel_pos, overwrite)
                        done_users[user_id] = true
                    end
                end
            end
        end
    end

    -- Merge role permissions, taking only the first found entry
    local new_role_perms = {}

    for _, data in lsd do
        for role_id, perms in data.role_permissions do
            if not new_role_perms[role_id] then
                new_role_perms[role_id] = perms
            end
        end
    end

    return {
        role_permissions = new_role_perms,
        channel_permissions = new_channel_perms,
    }
end

--- Returns the underlying permissions (or otherwise) for a given state
---
--- This works by merging the shareable data from all lockdowns in the order of least recent first,
--- to ensure that the permissions are derived from the least recent lockdowns first and then using
--- `f` to extract the desired data from the merged sharable data
local function getunderlying<T, U>(
    lockdowns: {mode.Lockdown},
    state: T,
    f: (handle.LockdownSharableData, T) -> U?
): U?
    table.sort(lockdowns, function(a: mode.Lockdown, b: mode.Lockdown): boolean
        return a.created_at < b.created_at
    end)

    -- Now loop over all lockdowns
    -- Because we sorted by creation date, the least recent lockdowns will be checked first
    -- hence ensuring the permissions we get are from before lockdowns
    local unmerged_lsds = {}
    for _, lockdown in lockdowns do
        table.insert(unmerged_lsds, lockdown.type.shareable(lockdown))
    end

    local merged_lsd = mergelsd(unmerged_lsds)

    return f(merged_lsd, state)
end

--- Returns the underlying role permissions if any
local function getunderlyingrolepermissions(
    lockdowns: {mode.Lockdown},
    role_id: discord.Snowflake
): string?
    return getunderlying(lockdowns, role_id, function(data: handle.LockdownSharableData, role_id: discord.Snowflake)
        return data.role_permissions[role_id]
    end)
end

--- Returns the underlying channel permission overwrites if any
local function getunderlyingchannelpermissions(
    lockdowns: {mode.Lockdown},
    channel_id: discord.Snowflake
): {discord.OverwriteObject}?
    return getunderlying(lockdowns, channel_id, function(data: handle.LockdownSharableData, channel_id: discord.Snowflake)
        return data.channel_permissions[channel_id]
    end)
end

--- Replaces a role in the guild roles list
local function replacerole(data: mode.BaseLockdownModeData, role: discord.GuildRoleObject, hint_idx: number?) 
    assert(data.guild.roles, "Internal error: Guild roles not found")

    if hint_idx then
        if data.guild.roles[hint_idx] and data.guild.roles[hint_idx].id == role.id then
            data.guild.roles[hint_idx] = role
            return
        end
    end

    for idx, obj in data.guild.roles do 
        if obj.id == role.id then 
            data.guild.roles[idx] = role 
            return
        end
    end

    -- If we couldn't update, then add
    table.insert(data.guild.roles, role)
end

--- Replaces a channel in the guild channel list
local function replacechannel(data: mode.BaseLockdownModeData, channel: discord.ChannelObject, hint_idx: number?) 
    if hint_idx then
        if data.channels[hint_idx] and data.channels[hint_idx].id == channel.id then
            data.channels[hint_idx] = channel
            return
        end
    end

    for idx, obj in data.channels do 
        if obj.id == channel.id then 
            data.channels[idx] = channel 
            return
        end
    end

    -- If we couldn't update, then add
    table.insert(data.channels, channel)
end

return {
    getCriticalRoles = getCriticalRoles,
    mergelsd = mergelsd,
    getunderlying = getunderlying,
    getunderlyingrolepermissions = getunderlyingrolepermissions,
    getunderlyingchannelpermissions = getunderlyingchannelpermissions,
    replacerole = replacerole,
    replacechannel = replacechannel,
}