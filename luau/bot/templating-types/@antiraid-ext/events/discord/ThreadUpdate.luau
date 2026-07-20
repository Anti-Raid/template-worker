local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- ThreadUpdate
---
--- Dispatched when a thread is updated.
local function ThreadUpdate(callback: (ctx: Primitives.TemplateContext, event: discordGateway.UpdateThreadPayload) -> ())
    return createTab("THREAD_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ThreadUpdate
