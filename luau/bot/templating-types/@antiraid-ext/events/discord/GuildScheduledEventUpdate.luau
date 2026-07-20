local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- GuildScheduledEventUpdate
---
--- Dispatched when a scheduled event is updated in a guild.
local function GuildScheduledEventUpdate(callback: (ctx: Primitives.TemplateContext, event: discord.GuildScheduledEventObject) -> ())
    return createTab("GUILD_SCHEDULED_EVENT_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildScheduledEventUpdate