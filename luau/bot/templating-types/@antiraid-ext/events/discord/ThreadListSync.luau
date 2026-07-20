local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- ThreadListSync
---
--- Dispatched when the current user gains access to a thread.
local function ThreadListSync(callback: (ctx: Primitives.TemplateContext, data: discordGateway.ThreadListSyncPayload) -> ())
    return createTab("THREAD_LIST_SYNC", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ThreadListSync
