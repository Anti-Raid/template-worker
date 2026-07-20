local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- StageInstanceDelete
---
--- Dispatched when a stage instance is deleted.
local function StageInstanceDelete(callback: (ctx: Primitives.TemplateContext, instance: discord.StageInstanceObject) -> ())
    return createTab("STAGE_INSTANCE_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return StageInstanceDelete
