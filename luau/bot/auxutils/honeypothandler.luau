local apitypes = require "@discord-types/apiTypes"
local Primitives = require "@antiraid-core/primitives"
local HoneypotManager = require "./honeypotmanager"

local honeypotManager: HoneypotManager.HoneypotManager? = nil
local function honeypotHandler(msg: apitypes.MessageObject, ctx: Primitives.TemplateContext): ()
    -- Honeypots
    if not msg.author then return end
    if not msg.content then return end

    if #msg.content < 10 then
        return -- Ignore short messages
    end

    local mentions = (msg.mentions or {}) :: {apitypes.UserObject}
    if #mentions > 5 then return end -- Only 3 mention allowed for AFK

    if not honeypotManager then
        honeypotManager = HoneypotManager(ctx)
    end
    assert(honeypotManager, "Internal error: HoneypotManager is nil")
    local honeypots = honeypotManager.list()
    for _, honeypot in honeypots do
        if msg.channel_id == honeypot.channelid then
            if msg.author.bot and honeypot.botallowed then break end -- Ignore clearly bot messages if bots are allowed in the honeypot
            if honeypot.punishment == "ban" then 
                -- First ban them
                ctx.discord:create_guild_ban({
                    reason = "Attempt to message in honeypot channel",
                    user_id = msg.author.id,
                })
                ctx.loop.dispatch({
                    name = "HoneypotBan",
                    data = {
                        userid = msg.author.id,
                        channelid = honeypot.channelid,
                        msgid = msg.id,
                    }
                })
            elseif honeypot.punishment == "kick" then
                -- First kick them
                ctx.discord:remove_guild_member({
                    reason = "Attempt to message in honeypot channel",
                    user_id = msg.author.id,
                })
                ctx.loop.dispatch({
                    name = "HoneypotKick",
                    data = {
                        userid = msg.author.id,
                        channelid = honeypot.channelid,
                        msgid = msg.id,
                    }
                })
            end
            break -- No need to check further honeypots
        end
    end
end

return honeypotHandler