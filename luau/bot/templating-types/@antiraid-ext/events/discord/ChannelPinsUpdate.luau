local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- ChannelPinsUpdate
local function ChannelPinsUpdate(callback: (ctx: Primitives.TemplateContext, pin: discordGateway.ChannelPinsUpdatePayload) -> ())
    return createTab("CHANNEL_PINS_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ChannelPinsUpdate