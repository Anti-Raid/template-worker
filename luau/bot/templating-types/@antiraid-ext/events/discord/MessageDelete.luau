local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- MessageDelete
---
--- Dispatched when a message is deleted.
local function MessageDelete(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.MessageDeletePayload) -> ())
    return createTab("MESSAGE_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return MessageDelete
