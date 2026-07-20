local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildIntegrationsUpdate
---
--- Dispatched when a guild’s integration is added, updated or removed.
local function GuildIntegrationsUpdate(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.GuildIntegrationsUpdatePayload) -> ())
    return createTab("GUILD_INTEGRATIONS_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildIntegrationsUpdate