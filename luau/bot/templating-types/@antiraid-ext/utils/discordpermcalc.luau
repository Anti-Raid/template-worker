local apitypes = require"@discord-types/apiTypes"
local permissions = require"@discord-types/permission"
local set = require"@antiraid-ext/set"

--- Computes the 'base permissions' of a member
--- @param g The guild object
--- @param m The member object
--- @return number The base permissions of the member
---
--- As this uses a different algorithm from the Rust layer, the permissions returned may be different if there are bugs
local function BasePermissions(g: apitypes.GuildObject, m: apitypes.GuildMemberObject): integer
    assert(m.user, "Member must have a user object to use BasePermissions")
    assert(g.roles, "Guild must have roles to use BasePermissions")
    if g.owner_id == m.user.id then
        return permissions.PermissionsAll
    end

    -- Get everyone role of the guild
    local everyoneRole = nil
    local roleMap = {}

    for _, role in g.roles do
        roleMap[role.id] = role
        if role.id == g.id then
            everyoneRole = role
        end
    end

    local perms = 0i

    -- Apply everyone role
    if everyoneRole then
        perms = integer.fromstring(everyoneRole.permissions, 10)
    end

    -- Apply member roles
    for _, roleId in m.roles do
        local role = roleMap[roleId]
        if role then
            perms = integer.bor(perms, integer.fromstring(role.permissions))
        end
    end

    if integer.band(perms, permissions.Permissions.Administrator) == permissions.Permissions.Administrator then
        return permissions.PermissionsAll -- ADMINISTRATOR overrides all
    end

    return perms
end

--- Computes the permissions of a member in a channel
--- @param basePerms The base permissions of the member
--- @param g The guild object
--- @param m The member object
--- @param c The channel object
--- @return integer The permissions of the member in the channel
local function ComputeMemberChannelOverwrites(basePerms: integer, g: apitypes.GuildObject, m: apitypes.GuildMemberObject, c: apitypes.ChannelObject): integer
    assert(m.user, "Member must have a user object to use ComputeMemberChannelOverwrites")
    if integer.band(basePerms, permissions.Permissions.Administrator) == permissions.Permissions.Administrator then
        return permissions.PermissionsAll -- ADMINISTRATOR overrides all
    end

    local perms = basePerms -- Start with base perms

    -- Now we have the base everyone perms, apply the rest of the perms in hierarchy order
    local everyoneOverwrite = nil
    local roleOverwrites = {}
    local memberOverwrite = nil
    for _, overwrite in c.permission_overwrites do
        if overwrite.type == permissions.OverwriteObjectTypeRole and overwrite.id == g.id then
            everyoneOverwrite = overwrite
        elseif overwrite.type == permissions.OverwriteObjectTypeRole then
            if table.find(m.roles, overwrite.id) then
                table.insert(roleOverwrites, overwrite)
            end
        elseif overwrite.type == permissions.OverwriteObjectTypeMember and overwrite.id == m.user.id then
            memberOverwrite = overwrite
        end
    end
    
    -- First, apply everyone overwrite
    if everyoneOverwrite then
        perms = integer.band(perms, integer.bnot(integer.fromstring(everyoneOverwrite.deny)))
        perms = integer.bor(perms, integer.fromstring(everyoneOverwrite.allow))
    end

    -- Next, apply role overwrites
    local allow = 0i
    local deny = 0i
    for _, overwrite in roleOverwrites do
        allow = integer.bor(allow, integer.fromstring(overwrite.allow))
        deny = integer.bor(deny, integer.fromstring(overwrite.deny))
    end

    perms = integer.band(perms, integer.bnot(deny))
    perms = integer.bor(perms, allow)

    -- Finally, apply member overwrite
    if memberOverwrite then
        perms = integer.band(perms, integer.bnot(integer.fromstring(memberOverwrite.deny)))
        perms = integer.bor(perms, integer.fromstring(memberOverwrite.allow))
    end 

    return perms
end

local function MemberChannelPerms(g: apitypes.GuildObject, m: apitypes.GuildMemberObject, c: apitypes.ChannelObject): integer
    local basePerms = BasePermissions(g, m)
    return ComputeMemberChannelOverwrites(basePerms, g, m, c)
end

--- Check if a permission u64 has a permission
--- @param perms integer The permission u64 to check
--- @param perm integer The permission to check for
--- @return boolean True if the permission is present, false otherwise
local function HasPermission(perms: integer, perm: integer): boolean
    -- Check if admin
    if integer.band(perms, permissions.Permissions.Administrator) == permissions.Permissions.Administrator then
        return true -- ADMINISTRATOR overrides all
    end
    return integer.band(perms, perm) == perm
end

--- Check if a permission u64 has all permissions in a set
--- @param perms integer The permission u64 to check
--- @param permSet set.Set<integer> The set of permissions to check for
--- @return boolean True if all permissions are present, false otherwise
local function HasAllPermissions(perms: integer, permSet: set.Set<integer>): boolean
    -- Check if admin
    if integer.band(perms, permissions.Permissions.Administrator) == permissions.Permissions.Administrator then
        return true -- ADMINISTRATOR overrides all
    end
    
    for perm in permSet do
        if integer.band(perms, perm) ~= perm then
            return false -- If any permission is missing, return false
        end
    end
    return true
end

return {
    BasePermissions = BasePermissions,
    ComputeMemberChannelOverwrites = ComputeMemberChannelOverwrites,
    MemberChannelPerms = MemberChannelPerms,
    HasPermission = HasPermission,
    HasAllPermissions = HasAllPermissions,
}