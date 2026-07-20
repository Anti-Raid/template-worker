local Primitives = require("@antiraid-core/primitives")
local discordGateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- MessagePollVoteAdd
---
--- Dispatched when a user votes in a poll.
---
--- This will be dispatched multiple times if multiple answers are selected.
local function MessagePollVoteAdd(callback: (ctx: Primitives.TemplateContext, vote: discordGateway.MessagePollVoteAdd) -> ())
    return createTab("MESSAGE_POLL_VOTE_ADD", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return MessagePollVoteAdd
