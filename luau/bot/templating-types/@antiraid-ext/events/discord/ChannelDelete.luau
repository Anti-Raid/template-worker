local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- ChannelDelete
local function ChannelDelete(callback: (ctx: Primitives.TemplateContext, channel: discord.ChannelObject) -> ())
    return createTab("CHANNEL_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ChannelDelete