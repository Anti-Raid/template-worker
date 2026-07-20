local apitypes = require"@discord-types/apiTypes"
local resttypes = require"@discord-types/restTypes"

export type ChannelChange = {
    type: "CreateChannel",
    --- The channel object to create
    channel: resttypes.CreateGuildChannelRequest
} | {
    type: "DeleteChannel",
    --- The ID of the channel to delete
    channelid: string
} | {
    type: "ModifyChannel",
    --- The ID of the channel to modify
    channelid: string,
    --- The channel object with the modified properties.
    channel: resttypes.ModifyChannelRequest
}

--- Converts a ChannelObject into a CreateGuildChannelRequest
local function channelToCreateRequest(ch: apitypes.ChannelObject): resttypes.CreateGuildChannelRequest
    return {
        name = ch.name,
        type = ch.type,
        topic = ch.topic,
        bitrate = ch.bitrate,
        user_limit = ch.user_limit,
        rate_limit_per_user = ch.rate_limit_per_user,
        position = ch.position,
        permission_overwrites = ch.permission_overwrites,
        parent_id = ch.parent_id,
        nsfw = ch.nsfw,
        rtc_region = ch.rtc_region,
        video_quality_mode = ch.video_quality_mode,
        default_auto_archive_duration = ch.default_auto_archive_duration,
        default_reaction_emoji = ch.default_reaction_emoji,
        available_tags = ch.available_tags,
        default_sort_order = ch.default_sort_order,
        default_forum_layout = ch.default_forum_layout,
        default_thread_rate_limit_per_user = ch.default_thread_rate_limit_per_user,
    }
end

--- Given `a` and `b`, returns if `a` is deep equal to `b`
local function innerEquals(a: any, b: any): boolean 
    if type(a) ~= type(b) then return false end

    if type(a) == "table" then
        if #a ~= #b then return false end

        local done = {}
        for key, value in a do
            if not innerEquals(value, b[key]) then return false end
            done[key] = true
        end
        for key, value in b do
            if done[key] then continue end
            if not innerEquals(a[key], value) then return false end
        end
        return true
    end
    return a == b
end

local function diff(old: {apitypes.ChannelObject}, new: {apitypes.ChannelObject}): {ChannelChange}
    local oldMap: {[string]: apitypes.ChannelObject} = {}
    local newMap: {[string]: apitypes.ChannelObject} = {}

    for _, channel in old do
        oldMap[channel.id] = channel
    end

    for _, channel in new do
        newMap[channel.id] = channel
    end

    local changes: {ChannelChange} = {}

    -- Create channels: need to revert and create channel b/c old is backup and new is
    -- current state
    for _, channel in old do
        if not newMap[channel.id] then
            table.insert(changes, {
                type = "CreateChannel" :: "CreateChannel",
                channel = channelToCreateRequest(channel)
            })
        end
    end

    -- Delete channels
    for _, channel in new do
        if not oldMap[channel.id] then
            table.insert(changes, {
                type = "DeleteChannel" :: "DeleteChannel",
                channelid = channel.id
            })
        end
    end

    -- Modify channels (deep diff all fields)
    for _, newCh in new do
        local oldCh = oldMap[newCh.id]
        if not oldCh then continue end -- ignore

        local channeldiff = {}
        local needschanges = false
        for key, value in (oldCh :: any) do 
            if not innerEquals((oldCh :: any)[key], (newCh :: any)[key]) then 
                channeldiff[key] = (oldCh :: any)[key]
                needschanges = true
            end
        end
        if needschanges then
            table.insert(changes, {
                type = "ModifyChannel" :: "ModifyChannel",
                channelid = newCh.id,
                channel = channeldiff :: resttypes.ModifyChannelRequest
            })
        end
    end

    return changes
end

return {
    diff = diff
}