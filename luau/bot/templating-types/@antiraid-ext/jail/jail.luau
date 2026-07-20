--!strict
--- Syscall filterer
local Primitives = require"@antiraid-core/primitives"
local runtime = require"@antiraid-core/plugins/runtime"
local setup = require"@antiraid-core/setup"
local Discord = require"@antiraid-ext/system/discord"

export type JailOpts = {
    ops: {string},
    isblacklist: boolean
}

local function isoptallowed(jo: JailOpts, op: string) 
    local idx = table.find(jo.ops, op)
    -- found+blacklist=true, found+not blacklist (whitelist) = false
    if not idx then return jo.isblacklist else return not jo.isblacklist end
end

export type JailConfig = {
    discord: JailOpts,
    denyunsupported: boolean
}

-- Creates a new 'jailed' context with syscall limits
local function createjail(rootctx: Primitives.TemplateContext, jc: JailConfig): Primitives.TemplateContext 
    local innerctxref: Primitives.TemplateContext
    local function jailcall(args: runtime.SyscallArgs): runtime.SyscallRet 
        assert(type(args) == "table", "syscall arguments must be a table")
        assert(getmetatable(args) == nil, "jailed context call with arg containing a metatable")
        if args.op == "Discord" then 
            if not isoptallowed(jc.discord, args.req.op) then 
                error(`jailed context call attempted blacklisted op '{args.req.op}'`)
            end
        elseif jc.denyunsupported then
            error("unsupported syscall op")
        end
        return rootctx.syscall(args)
    end

    local eventmanager: Primitives.EventManager
    local discord: Discord.DiscordClient
    innerctxref = table.freeze{
        syscall = jailcall,
        tenantstate = rootctx.tenantstate,
        btd = rootctx.btd,
        loop = eventmanager,
        discord = discord
    }

    eventmanager = setup.EventManager(innerctxref)
    discord = setup.Discord(innerctxref)

    local ctxany = innerctxref :: any
    ctxany.discord = discord
    ctxany.loop = eventmanager

    return innerctxref
end

return createjail