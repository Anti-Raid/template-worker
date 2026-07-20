local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- VoiceStateUpdate
---
--- Dispatched when a user joins, leaves or moves to a voice channel.
local function VoiceStateUpdate(callback: (ctx: Primitives.TemplateContext, new: discord.VoiceStateObject) -> ())
    return createTab("VOICE_STATE_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return VoiceStateUpdate
