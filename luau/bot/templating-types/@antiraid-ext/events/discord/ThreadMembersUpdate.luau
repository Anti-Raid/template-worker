local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- ThreadMembersUpdate
---
--- Dispatched when anyone is added to or removed from a thread.
local function ThreadMembersUpdate(callback: (ctx: Primitives.TemplateContext, data: discordGateway.ThreadMembersUpdatePayload) -> ())
    return createTab("THREAD_MEMBERS_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end 

return ThreadMembersUpdate
