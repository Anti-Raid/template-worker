local Primitives = require("@antiraid-core/primitives")
local createTab = require("@antiraid-ext/events/dispatch").createTab

export type FeedTicketRequestEvent = {
    topics: {string}
}

--- Triggered when a client wants to obtain a FeedTicket
local function FeedTicketRequest(callback: (ctx: Primitives.TemplateContext, evt: FeedTicketRequestEvent, author: string) -> any)
    return createTab("FeedTicketRequest", function(ctx, event)
        if not event.data then error("No data set on data-mandatory event") end
        return callback(ctx, event.data :: FeedTicketRequestEvent, event.author or error("No author set on author-mandatory event"))
    end)
end

return FeedTicketRequest
