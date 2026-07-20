local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- IntegrationUpdate
local function IntegrationUpdate(callback: (ctx: Primitives.TemplateContext, integration: discordgateway.IntegrationUpdatePayload) -> ())
    return createTab("INTEGRATION_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return IntegrationUpdate
 