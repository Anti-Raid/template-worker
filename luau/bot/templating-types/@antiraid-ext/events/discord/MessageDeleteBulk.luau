local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- MessageDeleteBulk
---
--- Dispatched when multiple messages are deleted at once.
local function MessageDeleteBulk(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.MessageDeleteBulkPayload) -> ())
    return createTab("MESSAGE_DELETE_BULK", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return MessageDeleteBulk
 