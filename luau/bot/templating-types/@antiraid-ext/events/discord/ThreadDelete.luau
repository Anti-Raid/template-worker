local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- ThreadDelete
---
--- Dispatched when a thread is deleted.
---
--- Provides the partial data about the deleted thread and, if it was present in the cache before its deletion, its full data.
local function ThreadDelete(callback: (ctx: Primitives.TemplateContext, event: discordGateway.DeleteThreadPayload) -> ())
    return createTab("THREAD_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ThreadDelete
