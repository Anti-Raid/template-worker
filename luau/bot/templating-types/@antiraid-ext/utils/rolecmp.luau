local discord = require("@discord-types/apiTypes")

--- Returns if A is lower than B in hierarchy order.
---
--- @param a discord.GuildRoleObject Role A
--- @param b discord.GuildRoleObject Role B
--- @return boolean ``true`` if A is lower than B in hierarchy order, ``false`` otherwise.
local function IsRoleALessThanRoleB(a: discord.GuildRoleObject, b: discord.GuildRoleObject): boolean
    assert(a.id, "Role A must have an ID")
    assert(b.id, "Role B must have an ID")

    if a.position == b.position then
        return a.id > b.id -- Newer roles are less than older roles
    end

    return a.position < b.position
end

--- Returns if A is less than or equal to B in hierarchy order.
---
--- @param a discord.GuildRoleObject Role A
--- @param b discord.GuildRoleObject Role B
--- @return boolean ``true`` if A is lower than or equal to B in hierarchy order, ``false`` otherwise.
local function IsRoleALessThanOrEqualToRoleB(a: discord.GuildRoleObject
, b: discord.GuildRoleObject): boolean
    assert(a.id, "Role A must have an ID")
    assert(b.id, "Role B must have an ID")

    if a.position == b.position then
        return a.id >= b.id -- Newer roles are less than or equal to older roles
    end

    return a.position < b.position
end

--- Returns if A is greater than B in hierarchy order.
---
--- @param a discord.GuildRoleObject Role A
--- @param b discord.GuildRoleObject Role B
--- @return boolean ``true`` if A is greater than or equal to B in hierarchy order, ``false`` otherwise.
local function IsRoleAGreaterThanRoleB(a: discord.GuildRoleObject, b: discord.GuildRoleObject): boolean
    assert(a.id, "Role A must have an ID")
    assert(b.id, "Role B must have an ID")

    if a.position == b.position then
        return a.id < b.id -- Older roles are greater than older roles
    end

    return a.position > b.position
end

--- Returns if Role A is greater than or equal to Role B in hierarchy order.
---
--- @param a discord.GuildRoleObject Role A
--- @param b discord.GuildRoleObject Role B
--- @return boolean ``true`` if A is greater than or equal to B in hierarchy order, ``false`` otherwise.
local function IsRoleAGreaterThanOrEqualToRoleB(a: discord.GuildRoleObject, b: discord.GuildRoleObject): boolean
    assert(a.id, "Role A must have an ID")
    assert(b.id, "Role B must have an ID")

    if a.position == b.position then
        return a.id <= b.id -- Older roles are greater than or equal to newer roles
    end

    return a.position > b.position
end

--- Sorts roles by hierarchy order in ascending order [lower roles first].
local function SortRolesByHierarchyAsc(roles: {discord.GuildRoleObject}): {discord.GuildRoleObject}
    table.sort(roles, function(a, b)
        return IsRoleALessThanRoleB(a, b)
    end)
    return roles
end

--- Sorts roles by hierarchy order in descending order [roles higher in hierarchy first].
local function SortRolesByHierarchyDesc(roles: {discord.GuildRoleObject}): {discord.GuildRoleObject}
    table.sort(roles, function(a, b)
        return IsRoleAGreaterThanRoleB(a, b)
    end)
    return roles
end

return {
    IsRoleALessThanRoleB = IsRoleALessThanRoleB,
    IsRoleALessThanOrEqualToRoleB = IsRoleALessThanOrEqualToRoleB,
    IsRoleAGreaterThanRoleB = IsRoleAGreaterThanRoleB,
    IsRoleAGreaterThanOrEqualToRoleB = IsRoleAGreaterThanOrEqualToRoleB,
    SortRolesByHierarchyAsc = SortRolesByHierarchyAsc,
    SortRolesByHierarchyDesc = SortRolesByHierarchyDesc,
}