local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- EntitlementUpdate
---
--- Provides data abut the updated subscription. If the entitlement is renewed, the ``entitlement.ends_at`` field will have changed.
local function EntitlementUpdate(callback: (ctx: Primitives.TemplateContext, entitlement: discord.EntitlementObject) -> ())
    return createTab("ENTITLEMENT_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return EntitlementUpdate