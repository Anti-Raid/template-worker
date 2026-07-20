local Primitives = require("@antiraid-core/primitives")

export type EventCall = {
    pat: string,
    cb: (Primitives.TemplateContext, Primitives.Event) -> any
}

local function createTab(pat: string, callback: (Primitives.TemplateContext, Primitives.Event) -> any): EventCall
    return {
        pat = pat,
        cb = callback
    }
end

local function eventList(...: EventCall): {[string]: (Primitives.TemplateContext, Primitives.Event) -> any}
    local tab = {}
    local args = {...}
    for _, map in args do 
        tab[map.pat] = map.cb
    end
    return tab
end

return {
    createTab = createTab,
    eventList = eventList
}