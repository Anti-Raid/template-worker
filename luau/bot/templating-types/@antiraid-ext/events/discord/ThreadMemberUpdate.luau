local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- ThreadMemberUpdate
---
--- Dispatched when a thread member is updated.
local function ThreadMemberUpdate(callback: (ctx: Primitives.TemplateContext, event: discordGateway.ThreadMemberUpdatePayload) -> ())
    return createTab("THREAD_MEMBER_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ThreadMemberUpdate
