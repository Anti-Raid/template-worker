local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- IntegrationCreate
---
--- Dispatched when an integration is created in a guild.
local function IntegrationCreate(callback: (ctx: Primitives.TemplateContext, integration: discordgateway.IntegrationCreatePayload) -> ())
    return createTab("INTEGRATION_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return IntegrationCreate
 