local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- StageInstanceCreate
---
--- Dispatched when a stage instance is created.
local function StageInstanceCreate(callback: (ctx: Primitives.TemplateContext, instance: discord.StageInstanceObject) -> ())
    return createTab("STAGE_INSTANCE_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return StageInstanceCreate
