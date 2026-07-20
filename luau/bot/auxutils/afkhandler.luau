--!strict
local apitypes = require "@discord-types/apiTypes"
local Primitives = require "@antiraid-core/primitives"
local managers = require "./managers/managers"

local function afkHandler(msg: apitypes.MessageObject, ctx: Primitives.TemplateContext): ()
    local afkmanager = managers.getmanagers(ctx).afkmanager
    -- AFKs
    if not msg.author then return end
    if msg.author.bot then return end -- Ignore bot messages

    if not msg.content or #(msg.embeds or {}) ~= 0 then return end

    if #msg.content < 10 then
        return -- Ignore short messages
    end

    local mentions = (msg.mentions or {}) :: {apitypes.UserObject}
    if #mentions > 5 then return end -- Only 3 mention allowed for AFK

    for _, mention in mentions do 
        local afk = afkmanager.get(mention.id)
        if not afk then continue end -- No AFK for this user
        ctx.discord:create_message({
            channel_id = msg.channel_id or error("msg.channel_id is nil"),
            data = {
                message_reference = {
                    channel_id = msg.channel_id or error("msg.channel_id is nil"),
                    message_id = msg.id,
                },  
                embeds = {
                    {
                        title = "AFK",
                        description = string.format("<@%s> is AFK: %s", mention.id, afk.reason),
                        color = 0x00FFFF,
                    }
                },
            },
        })
    end
end

return afkHandler