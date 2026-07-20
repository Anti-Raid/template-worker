local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildScheduledEventUserAdd
---
--- Dispatched when a user is added to a scheduled event in a guild.
local function GuildScheduledEventUserAdd(callback: (ctx: Primitives.TemplateContext, subscribed: discordGateway.GuildScheduledEventUserAddPayload) -> ())
    return createTab("GUILD_SCHEDULED_EVENT_USER_ADD", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildScheduledEventUserAdd
 