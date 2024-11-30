-- @pragma {"lang":"lua","allowed_caps":["discord:create_message","kv:*"]}
local args, token = ...
local discord = require "@antiraid/discord"
local interop = require "@antiraid/interop"
local kv = require "@antiraid/kv"

-- Simple trim function
function trim_string(s: string): string
    return (s:gsub("^%s*(.-)%s*$", "%1"))
end

-- Trim check and split by space
function split_string(s: string): {string}
    local t = string.split(s, " ")

    for i, v in ipairs(t) do
        t[i] = trim_string(v)
    end

    return t
end

if args.event_name ~= "MESSAGE" then
    return -- Early return if not a message event
end

-- Get the user ID from the event data
local user_id: string = args.event_data["new_message"]["author"]["id"]
local channel_id: string = args.event_data["new_message"]["channel_id"]
local content: string = args.event_data["new_message"]["content"]
local mention_users = args.event_data["new_message"]["mentions"]
local antiraid_userid: string = interop.current_user()["id"]

-- Map the user to ID in mention users
for i, v in ipairs(mention_users) do
    mention_users[i] = v["id"]
end

if table.find(mention_users, antiraid_userid) then
    local content_split = split_string(content)

    if content_split[2] == "afk" then
        -- The duration (in seconds) is optionally present in second argument
        local duration = 60 * 60 -- Default to 1 hour

        if #content_split > 2 then
            local duration_str = content_split[3]
            local duration_num = tonumber(duration_str)

            if duration_num then
                duration = duration_num
            end
        end

        -- Set the user as AFK
        local kv_executor = kv.new(token)
        kv_executor:set("afk/" .. user_id, os.time() + duration)

        -- Send message
        local discord_executor = discord.new(token);
        discord_executor:create_message({
            channel_id = channel_id,
            message = {
                content = "<@" .. user_id .. "> is now AFK for " .. duration .. " seconds"
            }
        })
    end
end

