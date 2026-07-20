local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- StageInstanceUpdate
---
--- Dispatched when a stage instance is updated.
local function StageInstanceUpdate(callback: (ctx: Primitives.TemplateContext, new: discord.StageInstanceObject) -> ())
    return createTab("STAGE_INSTANCE_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return StageInstanceUpdate
 