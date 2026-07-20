local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildScheduledEventCreate
---
--- Dispatched when a scheduled event is created in a guild.
local function GuildScheduledEventCreate(callback: (ctx: Primitives.TemplateContext, event: discord.GuildScheduledEventObject) -> ())
    return createTab("GUILD_SCHEDULED_EVENT_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildScheduledEventCreate