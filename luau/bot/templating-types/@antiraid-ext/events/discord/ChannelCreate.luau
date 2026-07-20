local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- ChannelCreate
local function ChannelCreate(callback: (ctx: Primitives.TemplateContext, channel: discord.ChannelObject) -> ())
    return createTab("CHANNEL_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ChannelCreate