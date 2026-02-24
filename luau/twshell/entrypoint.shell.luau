--~strict
local shellcontext = require"./shell/shellcontext"
local createmodhelpers = require"./shell/modhelpers"
local luau = require"@antiraid/luau"
local typesext = require"@antiraid/typesext"

local function docall(input: string, evalctx: any): any
    local chunk = luau.load(input)
    chunk.chunk_name = "/init.luau"
    chunk.environment = evalctx
    chunk.optimization_level = 2
    return chunk:call_async()
end

local function xptrace(args: any) 
    return debug.traceback(tostring(args), 1)
end

local function main(ctx: shellcontext.ShellContext) 
    ctx:Log("Fauxpas shell")

    -- Create a global proxy for the eval context to prevent pollution of the global environment and allow for storing last results etc
    local evalctx = typesext.createglobalproxy()
    evalctx.ctx = ctx
    evalctx.last = nil :: any
    evalctx.lasterror = nil :: any
    evalctx.mod = createmodhelpers(ctx)

    while true do 
        local input = ctx:GetInput("> ")
        if input == nil then break end -- EOF
        if input == "exit" then
            ctx:Log("Exiting shell.")
            break
        else
            local ok, res = xpcall(docall, xptrace, `return {input}`, evalctx)

            if not ok and string.find(tostring(res), "syntax error") then 
                -- Try again without the `return` in case it's a statement and not an expression
                ok, res = xpcall(docall, xptrace, input, evalctx)
            end

            if ok then 
                ctx:Log("Ok:", res)
                evalctx.last = res
            else
                ctx:Log("Error:", res)
                evalctx.lasterror = res
            end
        end
    end
end
return main