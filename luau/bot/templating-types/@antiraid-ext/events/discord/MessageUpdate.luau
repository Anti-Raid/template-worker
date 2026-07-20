local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- MessageUpdate
---
--- Provides the message update data, as well as the actual old and new message if data is available.
local function MessageUpdate(callback: (ctx: Primitives.TemplateContext, payload: discordGateway.MessageUpdatePayload) -> ())
    return createTab("MESSAGE_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return MessageUpdate
