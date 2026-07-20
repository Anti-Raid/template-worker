local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- GuildStickersUpdate
---
--- Dispatched when the stickers in a guild are updated.
local function GuildStickersUpdate(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.GuildStickersUpdatePayload) -> ())
    return createTab("GUILD_STICKERS_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildStickersUpdate
 