-- @pragma {"lang":"lua","allowed_caps":["discord:create_message"]}
local args, token = ...
local discord = require "@antiraid/discord"
local interop = require "@antiraid/interop"
local serde = require "@lune/serde"

-- Make the embed
local embed = {
    title = args.event_titlename, 
    description = "", -- Start with empty description
}

-- Add the event data to the description
for key, value in pairs(args.event_data) do
    local should_set = false

    if value ~= interop.null then
        should_set = true
    end

    if should_set then
        local formatted_value = serde.encode('json', value)
        embed.description = embed.description .. "**" .. key:gsub("_", " "):upper() .. "**: " .. formatted_value .. "\n"
    end
end

local message = { embeds = {} }
setmetatable(message.embeds, interop.array_metatable)

table.insert(message.embeds, embed)

-- Send message using action executor
local discord_executor = discord.new(token);
discord_executor:create_message({
    channel_id = args.sink,
    message = message
})
