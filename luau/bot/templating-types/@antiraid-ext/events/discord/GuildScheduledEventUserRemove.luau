local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildScheduledEventUserRemove
---
--- Dispatched when a user is removed from a scheduled event in a guild.
local function GuildScheduledEventUserRemove(callback: (ctx: Primitives.TemplateContext, unsubscribed: discordGateway.GuildScheduledEventUserRemovePayload) -> ())
    return createTab("GUILD_SCHEDULED_EVENT_USER_REMOVE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildScheduledEventUserRemove
 