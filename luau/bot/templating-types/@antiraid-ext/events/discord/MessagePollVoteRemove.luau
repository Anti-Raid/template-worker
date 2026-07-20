local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- MessagePollVoteRemove
---
--- Dispatched when a user removes their vote in a poll.
local function MessagePollVoteRemove(callback: (ctx: Primitives.TemplateContext, vote: discordGateway.MessagePollVoteRemove) -> ())
    return createTab("MESSAGE_POLL_VOTE_REMOVE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return MessagePollVoteRemove
