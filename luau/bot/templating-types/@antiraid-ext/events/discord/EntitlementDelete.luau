local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- EntitlementDelete
---
--- Provides data about the subscription. Specifically, the ``entitlement.deleted`` field will be set.
local function EntitlementDelete(callback: (ctx: Primitives.TemplateContext, entitlement: discord.EntitlementObject) -> ())
    return createTab("ENTITLEMENT_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return EntitlementDelete