local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildScheduledEventDelete
---
--- Dispatched when a scheduled event is deleted in a guild.
local function GuildScheduledEventDelete(callback: (ctx: Primitives.TemplateContext, event: discord.GuildScheduledEventObject) -> ())
    return createTab("GUILD_SCHEDULED_EVENT_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildScheduledEventDelete