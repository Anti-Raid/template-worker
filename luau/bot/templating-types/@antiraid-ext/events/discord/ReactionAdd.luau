local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- ReactionAdd
---
--- Dispatched when a reaction is added to a message.
local function ReactionAdd(callback: (ctx: Primitives.TemplateContext, reaction: discordGateway.MessageReactionAddPayload) -> ())
    return createTab("MESSAGE_REACTION_ADD", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ReactionAdd
 