local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- ThreadCreate
---
--- Dispatched when a thread is created or the current user is added to a private thread.
local function ThreadCreate(callback: (ctx: Primitives.TemplateContext, event: discordGateway.CreateThreadPayload) -> ())
    return createTab("THREAD_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ThreadCreate
