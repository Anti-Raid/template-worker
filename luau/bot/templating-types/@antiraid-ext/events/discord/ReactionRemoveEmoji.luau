local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- ReactionRemoveEmoji
---
--- Dispatched when all reactions of a specific emoji are removed from a message.
local function ReactionRemoveEmoji(callback: (ctx: Primitives.TemplateContext, payload: discordGateway.MessageReactionRemoveEmojiPayload) -> ())
    return createTab("MESSAGE_REACTION_REMOVE_EMOJI", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return ReactionRemoveEmoji
