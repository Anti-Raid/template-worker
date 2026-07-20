local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
  
--- VoiceServerUpdate
---
--- Dispatched when a guild’s voice server was updated (or changed to another one).
---
--- Note that you usually won't get this event from AntiRaid unless you're self-hosting or unusual circumstances.
local function VoiceServerUpdate(callback: (ctx: Primitives.TemplateContext, data: discordGateway.VoiceServerUpdatePayload) -> ())
    return createTab("VOICE_SERVER_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return VoiceServerUpdate
