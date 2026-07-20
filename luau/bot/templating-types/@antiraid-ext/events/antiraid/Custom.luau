local Primitives = require("@antiraid-core/primitives")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- Run a callback when the specified event is triggered
local function customEvent(eventname: string) 
    local function custom(callback: (ctx: Primitives.TemplateContext, event: any) -> ())
        return createTab(eventname, function(ctx, event)
            return callback(ctx, event.data)
        end)
    end

    return custom
end

return customEvent