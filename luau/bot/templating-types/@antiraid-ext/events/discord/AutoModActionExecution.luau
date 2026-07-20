local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- AutoModActionExecution
---
--- Note that discord-luau calls it ApplicationCommandPermissionsExecutionPayload instead of AutoModActionExecution due to naming bug
local function AutoModActionExecution(callback: (ctx: Primitives.TemplateContext, action: discordGateway.ApplicationCommandPermissionsExecutionPayload) -> ())
    return createTab("AUTO_MODERATION_ACTION_EXECUTION", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return AutoModActionExecution