local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
  
--- InviteDelete
---
--- Dispatched when an invite is deleted.
local function InviteDelete(callback: (ctx: Primitives.TemplateContext, data: discordGateway.InviteDeletePayload) -> ())
    return createTab("INVITE_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return InviteDelete
