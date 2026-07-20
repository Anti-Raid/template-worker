local apiTypes = require "@discord-types/apiTypes"

--- Returns the avatar URL for a user or returns a default avatar URL if the user does not have a custom avatar.
---
--- @param user apiTypes.UserObject
--- @return string The URL of the user's avatar.
local function avatarUrl(user: apiTypes.UserObject)
    if user.avatar then 
        return "https://cdn.discordapp.com/avatars/" .. user.id .. "/" .. user.avatar .. ".png"
    end

    local index = 0
    if user.discriminator == "0" or user.discriminator == "0000" then
        index = tostring((tonumber(user.discriminator) or 0) % 5)
    else
        index = tostring(integer.urem(integer.rshift(integer.fromstring((user :: any).id), 22i), 6i)) -- ((user_id >> 22) % 6)
    end

    local avatarUrl = "https://cdn.discordapp.com/embed/avatars/" .. (index) .. ".png"
    return avatarUrl
end

--- Returns the guild icon URL for a given guild object or a default avatar URL if the guild does not have an icon.
---
--- @param guild apiTypes.GuildObject
--- @return string The URL of the guild's
local function guildIconUrl(guild: apiTypes.GuildObject): string
    if guild.icon then
        return "https://cdn.discordapp.com/icons/" .. guild.id .. "/" .. guild.icon .. ".png" :: any
    end
    return "https://cdn.discordapp.com/embed/avatars/0.png"
end

return {
    avatarUrl = avatarUrl,
    guildIconUrl = guildIconUrl,
}