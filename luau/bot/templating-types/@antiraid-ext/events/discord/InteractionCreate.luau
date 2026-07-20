local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- InteractionCreate
---
--- Run an event on interaction create (e.g a slash command was used or a button was clicked)
local function InteractionCreate(callback: (ctx: Primitives.TemplateContext, interaction: discord.InteractionObject) -> ())
    return createTab("INTERACTION_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return InteractionCreate
