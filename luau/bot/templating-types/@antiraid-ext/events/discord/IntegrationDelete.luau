local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- IntegrationDelete
---
--- Dispatched when an integration is deleted in a guild.
local function IntegrationDelete(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.IntegrationDeletePayload) -> ())
    return createTab("INTEGRATION_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return IntegrationDelete
 