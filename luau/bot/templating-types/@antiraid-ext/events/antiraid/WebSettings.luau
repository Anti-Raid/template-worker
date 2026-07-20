local Primitives = require("@antiraid-core/primitives")
local settings = require("@antiraid-core/settings")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- WebSettings
local function WebSettings(callback: (ctx: Primitives.TemplateContext, evt: settings.Event, author: string) -> any)
    return createTab("WebSettings", function(ctx, event)
        if not event.data then error("No data set on data-mandatory event") end
        return callback(ctx, event.data, event.author or error("No author set on author-mandatory event"))
    end)
end

return WebSettings