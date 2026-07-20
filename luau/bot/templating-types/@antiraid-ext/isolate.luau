--!strict
local Primitives = require"@antiraid-core/primitives"
local typesext = require"@antiraid/typesext"
local luau = require"@antiraid/luau"
local LazyLoad = require"@antiraid-ext/sync/lazyload"

export type Isolate = {
    --- The ID of the isolate.
    read id: string,

    --- The global table of the isolate.
    read global_table: {[any]: any},

    --- Fires an event 'in-thread' and returns the result directly.
    read runEvent: (rootctx: Primitives.TemplateContext, event: Primitives.Event) -> any,
}

--- Creates a new isolate with its own global table and require function
---
--- The `expose` parameter can be used to expose additional values to the isolate's global table
local function new(id: string, vfs: typesext.Vfs, expose: {[any]: any}): Isolate
    local isolate = {}
    
    local globaltable: {[any]: any} = typesext.createglobalproxy()

    -- The require function will automatically cache modules per-isolate and ensure functions are loaded with the proxyglobal global table
    -- as the chunk's environment
    local requirefunc = vfs:createrequirefunction(id, globaltable) 
    
    -- Create a proxy require method with /init.luau as the chunk name
    --
    -- Required for require to start requiring from /
    local proxyrequirefuncchunk = luau.load("local requirer = ...; return function(...) return requirer(...) end")
    proxyrequirefuncchunk.chunk_name = "/init.luau"
    local proxyrequirefunc = proxyrequirefuncchunk:call(requirefunc)
    
    globaltable.require = requirefunc -- Expose our custom require function

    -- Expose any additional values to the isolate's global table
    for k, v in expose do
        globaltable[k] = v
    end

    local entrypointloader = LazyLoad(function() 
        return proxyrequirefunc("./init")
    end)

    isolate.id = id
    isolate.global_table = globaltable
    isolate.runEvent = function(rootctx: Primitives.TemplateContext, event: Primitives.Event): any 
        local entrypoint = entrypointloader.get()

        if type(entrypoint) ~= "table" then
            error(`init.luau for template \`{id}\` did not return a table, got \`{type(entrypoint)}\``)
        end
        local func: unknown = entrypoint[event.name]
        if func and type(func) == "function" then 
            return (func :: (Primitives.TemplateContext, Primitives.Event) -> any)(rootctx, event)
        else 
            return nil
        end
    end

    table.freeze(isolate)

    return isolate
end

return {
    new = new,
}