local Primitives = require"@antiraid-core/primitives"
local apitypes = require"@discord-types/apiTypes"
local channel = require"@discord-types/channel"
local types = require"../backups"
local channelallocs = require"../channelallocs"
local set = require"@antiraid-ext/set"
local sb = require"@antiraid-ext/utils/statusbuffer"

local function coreBackup(ctx: Primitives.TemplateContext, opts: types.BackupCreateOpts, updater: sb.StatusBuffer): types.CoreBackupData 
    local guild = ctx.discord:get_guild().data
    local channels = ctx.discord:get_guild_channels().data

    if not guild.roles or #guild.roles == 0 then 
        local roles = ctx.discord:get_guild_roles().data
        local rolesArr = {}
        for _, role in roles do table.insert(rolesArr, role) end
        guild.roles = rolesArr
    end

    local messages = {}
    local channelAllocationsFinal = {}
    if opts.backupMessages then 
        updater.addStatus("Backing up messages with perChannel of " .. opts.perChannel .. " and maxMessages of " .. opts.maxMessages)
        local botMember = ctx.discord:get_guild_member(ctx.btd().bot.id).data
        local channelAllocations = channelallocs.CreateChannelAllocations(
            guild, 
            botMember,
            channels,
            opts.channels,
            set.Set<<integer>>(),
            {
                channel.ChannelTypesMap.GuildText,
                channel.ChannelTypesMap.GuildAnnouncement,
                channel.ChannelTypesMap.AnnouncementThread,
                channel.ChannelTypesMap.PublicThread,
                channel.ChannelTypesMap.PrivateThread,
                channel.ChannelTypesMap.GuildForum,
            },
            opts.specialAllocations,
            opts.perChannel,
            opts.maxMessages
        )

        channelAllocationsFinal = channelAllocations.allocations

        messages = channelallocs.FetchAllocatedMessages(
            channelAllocations,
            function(channelId: string, count: number): {apitypes.MessageObject}
                updater.addStatus(`Fetching '{count}' messages from channel '{channelId}'...`)

                local messagesDiscord = ctx.discord:get_channel_messages({
                    channel_id = channelId,
                    limit = count,
                }).data

                updater.addStatus(`Fetched {#messagesDiscord} messages so far from channel {channelId}`)

                return messagesDiscord
            end,
            opts.maxMessages
        ) :: any
    end

    return {
        guild = guild,
        channels = channels,
        messages = messages,
        options = opts,
        channel_allocation = channelAllocationsFinal,
    }
end

return coreBackup