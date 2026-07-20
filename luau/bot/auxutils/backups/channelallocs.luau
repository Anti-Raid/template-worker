local apitypes = require"@discord-types/apiTypes"
local set = require"@antiraid-ext/set"
local discordpermcalc = require"@antiraid-ext/utils/discordpermcalc"

local function GetChannelsFromList(channels: {apitypes.ChannelObject}, filterChans: {string}): {apitypes.ChannelObject}
    if #filterChans == 0 then
        return channels
    end

    -- Store all channels selected in a hashmap
    local hasChannels = {}
    for _, c in filterChans do
        hasChannels[c] = true
    end

    -- Now filter out the channels
    local chans = {}
    for _, c in channels do
        if not hasChannels[c.id] then
            continue
        end
        table.insert(chans, c)
    end

    return chans
end

export type ChannelAllocationMap = {
    --- A map of channel ID to message count
    allocations: {[string]: number}, 
    --- The total number of messages allocated across all channels
    totalCount: number, 
    --- Add a message allocation to a channel
    setAllocation: (channelId: string, count: number) -> (),
    --- Remove a message allocation from a channel
    removeAllocation: (channelId: string) -> (),
}

-- Internal structure for channel allocations
local function ChannelAllocationMap(cap: number): ChannelAllocationMap 
    local self = {}
    self.allocations = {} :: {[string]: number}
    self.totalCount = 0

    self.setAllocation = function(channelId: string, count: number)
        local currentCount = self.allocations[channelId] or 0
        self.allocations[channelId] = count
        self.totalCount += count - currentCount
        if self.totalCount < 0 then
            error("Total count cannot be negative")
        end
        if self.totalCount > cap then
            error(`Total count cannot exceed the maximum capacity of {cap}`)
        end
    end

    self.removeAllocation = function(channelId: string)
        local currentCount = self.allocations[channelId] or 0
        self.totalCount -= currentCount
        self.allocations[channelId] = nil
    end

    return self 
end

--- Creates channel allocations for a guild based on the provided channels and options
--- @param g The guild object
--- @param bot The bot member object in the guild   
--- @param channels The list of channels to allocate messages to
--- @param neededPerms The set of permissions needed for the bot to operate in the channels
--- @param allowedChannelTypes The list of channel types that are allowed for allocation
--- @param specialAllocations A map of channel IDs to message counts for special allocations
--- @param perChannel The number of messages to allocate per channel
--- @param maxMessages The maximum number of messages to allocate across all channels
--- @return ChannelAllocationMap The channel allocation map with the allocations set
local function CreateChannelAllocations(
    g: apitypes.GuildObject,
    bot: apitypes.GuildMemberObject,
    channels: {apitypes.ChannelObject},
    channelList: {string}, -- List of channel IDs to allocate messages to
    neededPerms: set.Set<integer>,
    allowedChannelTypes: {apitypes.ChannelType},
    specialAllocations: {[string]: number},
    perChannel: number,
    maxMessages: number
): ChannelAllocationMap 
    local basePerms = discordpermcalc.BasePermissions(g, bot)

    -- Create channel map to allow for easy channel lookup
    local channelMap = {}
    for _, c in channels do
        if not table.find(allowedChannelTypes, c.type) then
            continue -- Skip channels that are not of an allowed type
        end

        if #channelList > 0 and not table.find(channelList, c.id) then
            continue -- Skip channels that are not in the channel list
        end

        channelMap[c.id] = {
            channel = c,
            perms = discordpermcalc.ComputeMemberChannelOverwrites(basePerms, g, bot, c),
        }
    end

    -- Create the channel allocation map
    local channelAllocations: ChannelAllocationMap = ChannelAllocationMap(maxMessages)

    -- First, handle special allocations
    for channelId, count in specialAllocations do
        local channelData = channelMap[channelId]
        if not channelData then 
            error(`Channel {channelId} not found in guild {g.id} for special allocation to it (or may not be of an allowed type)`)
        end

        if not discordpermcalc.HasAllPermissions(channelData.perms, neededPerms) then
            error(`Bot does not have all needed permissions in channel {channelId} for special allocation to it`)
        end

        if count <= 0 then
            error(`Special allocation for channel {channelId} must be greater than 0`)
        end

        -- Ensure we don't exceed the max messages limit
        if channelAllocations.totalCount + count > maxMessages then
            error(`Special allocation for channel {channelId} would exceed max messages limit of {maxMessages}`)
        end

        channelAllocations.setAllocation(channelId, count)
    end

    -- Then handle the rest of the channels
    for _, channelData in channelMap do 
        if channelAllocations.allocations[channelData.channel.id] then
            continue -- Skip channels that already have an (special) allocation set
        end

        if not discordpermcalc.HasAllPermissions(channelData.perms, neededPerms) then
            continue -- Skip channels where the bot does not have all needed permissions
        end

        -- Let remainingAllocations be the remaining messages we can allocate (maxMessages - channelAllocations.totalCount)
        -- If perChannel > remainingAllocations, we can only allocate the remainingAllocations
        -- Otherwise, we allocate perChannel messages
        local allocCount = math.min(perChannel, maxMessages - channelAllocations.totalCount)

        if allocCount <= 0 then
            continue -- No more messages can be allocated
        end

        channelAllocations.setAllocation(channelData.channel.id, allocCount)
    end

    return channelAllocations
end

--- Given an initial *valid* channel allocation map/plan (e.g. created using `CreateChannelAllocations`) and a callback to fetch as many messages as possible,
--- this function will attempt to allocate messages to channels based on the plan.
---
--- @param channelAllocations The initial channel allocation map
--- @param fetchMessagesCallback A callback function that takes a channel ID and returns a list of
--- @param maxMessages The maximum number of messages to fetch in total
--- @return {[string]: {apitypes.MessageObject}} A map of channel IDs to lists of messages fetched from those channels
local function FetchAllocatedMessages(
    channelAllocations: ChannelAllocationMap,
    fetchMessagesCallback: (channelId: string, count: number) -> {apitypes.MessageObject},
    maxMessages: number
): {[string]: {apitypes.MessageObject}}
    local totalHandledMessages = 0
    local messagesByChannel = {}

    for channelid, count in channelAllocations.allocations do
        assert(type(channelid) == "string", `Channel ID must be a string, got {type(channelid)}`)
        if count <= 0 then
            continue -- Skip channels with no messages allocated
        end

        local messages = fetchMessagesCallback(channelid, count)
        assert(messages, `internal error: fetchMessagesCallback for channel {channelid} returned nil`)

        totalHandledMessages += #messages
        messagesByChannel[channelid] = messages

        assert(
            totalHandledMessages <= maxMessages, 
            `internal error: total handled messages {totalHandledMessages} exceeded max messages limit of {maxMessages}`
        )
    end

    return messagesByChannel
end

return {
    GetChannelsFromList = GetChannelsFromList,
    CreateChannelAllocations = CreateChannelAllocations,
    FetchAllocatedMessages = FetchAllocatedMessages,
}