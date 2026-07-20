local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- InviteCreate
---
--- Dispatched when an invite is created.
local function InviteCreate(callback: (ctx: Primitives.TemplateContext, data: discordGateway.InviteCreatePayload) -> ())
    return createTab("INVITE_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return InviteCreate
