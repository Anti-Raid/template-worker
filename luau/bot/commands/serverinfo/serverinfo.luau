local commandBuilder = require "@discord-types/builders/interaction/interaction"
local NewSnowflake = require "@discord-types/snowflake"
local apitypes = require "@discord-types/apiTypes"
local compare = require "@antiraid-ext/utils/rolecmp"
local guildIconUrl = require "@antiraid-ext/utils/avatar".guildIconUrl
local context = require"@antiraid-ext/frameworkv2/context"

local function getCreationTimestampFromSnowflake(snowflake: string): number
    return NewSnowflake.new(snowflake):getTimestamp().timestamp_seconds
end

local command = commandBuilder.new({
    name = "serverinfo",
})
:addIntegrationType("GuildInstall") 
:setType("ChatInput")
:addContext("Guild")
:setDescription("Shows information about this server/guild")
:build()

local function run(data: context.CommandContext)
    local guild: apitypes.GuildObject = data.ctx.discord:get_guild().data 

    local guildId = guild.id or error("Guild ID not found in context")
    local channels: {apitypes.ChannelObject} = data.ctx.discord:get_guild_channels().data
    local roles: {apitypes.GuildRoleObject} = guild.roles or {}
    local emojis: {apitypes.EmojiObject} = guild.emojis or {}

    local textChannels, voiceChannels, categories, threads, newsChannels = 0, 0, 0, 0, 0
    for _, ch in channels do
        if ch.type == 0 then textChannels += 1
        elseif ch.type == 2 then voiceChannels += 1
        elseif ch.type == 4 then categories += 1
        elseif ch.type == 5 then newsChannels += 1
        elseif ch.type == 10 or ch.type == 11 or ch.type == 12 then threads += 1
        end
    end

    local animatedEmojis, staticEmojis = 0, 0
    for _, emoji in emojis do
        if emoji.animated then
            animatedEmojis += 1
        else
            staticEmojis += 1
        end
    end

    local highestRole: apitypes.GuildRoleObject? = nil
    for _, r in roles do
        if not highestRole or compare.IsRoleALessThanOrEqualToRoleB(highestRole, r) then
            highestRole = r
        end
    end

    --local memberCount = preview and preview.data and preview.data.approximate_member_count or guild.member_count or 0
    local boostLevel = tostring(guild.premium_tier or "None")
    local boostCount = guild.premium_subscription_count or 0
    local verificationLevel = guild.verification_level or "Unknown"
    local features = guild.features and table.concat(guild.features, ", ") or "None"
    local creationTimestamp = getCreationTimestampFromSnowflake(guildId)

    local afkTimeout = typeof(guild.afk_timeout) == "number" and (guild.afk_timeout / 60) or 0
    local afkChannel = if guild.afk_channel_id then string.format("<#%s>", guild.afk_channel_id) else "None"
    local nsfwLevel = guild.nsfw_level or "Unknown"

    local totalMemberCount = tostring(guild.approximate_member_count or "Unknown")

    local embed: apitypes.EmbedObject = {
        title = guild.name or "Server Info",
        description = string.format(
            "ID: `%s`\nOwner: <@%s>\nCreated: <t:%d:F>",
            guildId,
            guild.owner_id or "Unknown",
            creationTimestamp
        ),
        fields = {
            { name = "Highest Role", value = highestRole and string.format("<@&%s>", highestRole.id) or "Unknown", inline = true },
            { name = "Roles", value = tostring(#roles), inline = true }, 
            { name = "Text Channels", value = tostring(textChannels), inline = true },
            { name = "Voice Channels", value = tostring(voiceChannels), inline = true },
            { name = "Categories", value = tostring(categories), inline = true },

            { name = "Static Emojis", value = tostring(staticEmojis), inline = true },
            { name = "Animated Emojis", value = tostring(animatedEmojis), inline = true },
            { name = "Stickers", value = tostring(if guild.stickers then #guild.stickers else "Unknown"), inline = true },

            { name = "Boosts", value = string.format("Level %s (%d boosts)", boostLevel, boostCount), inline = true },
            { name = "Verification", value = tostring(verificationLevel), inline = true },
            { name = "NSFW Level", value = tostring(nsfwLevel), inline = true },

            { name = "AFK Channel", value = afkChannel, inline = true },
            { name = "AFK Timeout", value = string.format("%d minutes", afkTimeout), inline = true },

            { name = "Features", value = features, inline = false },
            { name = "Total Members", value = totalMemberCount, inline = true },
        },
        thumbnail = { url = guildIconUrl(guild) or "" },
    }

    data.interaction.reply({
        embeds = {embed}
    })
end

return {
    command = command,
    run = run,
}
