local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- ReactionRemoveAll
---
--- Dispatched when all reactions are removed from a message.
local function ReactionRemoveAll(callback: (ctx: Primitives.TemplateContext, reaction: discordGateway.MessageReactionRemoveAllPayload) -> ())
    return createTab("MESSAGE_REACTION_REMOVE_ALL", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ReactionRemoveAll
