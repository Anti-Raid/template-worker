--!strict

local Primitives = require"@antiraid-core/primitives"
local runtime = require"@antiraid-core/plugins/runtime"
local isolate = require"@antiraid-ext/isolate"
local MutexFn = require"@antiraid-ext/sync/mutex"
local typesext = require"@antiraid/typesext"
local Discord = require"@antiraid-ext/system/discord"

-- Flags 

-- Other constants

-- Start code

--- @noyield
local function EventManager(ctx: Primitives.TemplateContext): Primitives.EventManager
    local eventrefmu = MutexFn() -- Mutex for synchronizing access to event refs
    local exposedvfs = ctx.btd().base_vfs
    local attachedisolates: {[string]: Primitives.Dispatchable} = {
        ["builtins"] = isolate.new("builtins", typesext.Vfs.newoverlay({
            exposedvfs.Builtins or error("Internal error: Builtins VFS not found in exposed VFSs"),
            exposedvfs.TemplatingTypes or error("Internal error: TemplatingTypes VFS not found in exposed VFSs"),
        }), {
            print = function(...)
                local str = (typesext.fmtpretty :: any)(...)
                ctx.stream.publish("print", { type = "Log", message = str, source = "builtins" })
            end
        })
    } -- map of all attached isolates
    local attachedisolatescount = 1 -- initially only one attached isolate

    local function isSubscribed(event: string, system: string): boolean 
        local ts = ctx.tenantstate()
        return ts.events[event] ~= nil and ts.events[event][system] ~= nil
    end

    local function subscribe(event: string, system: string, attachedop: runtime.StateOp?) 
        eventrefmu.call(function() 
            if isSubscribed(event, system) and not attachedop then return nil end
            local ops: {runtime.StateOp} = {}

            if not isSubscribed(event, system) then 
                table.insert(ops, {
                    op = "SubscribeEvent" :: "SubscribeEvent",
                    event = event,
                    system = system
                })
            end
            if attachedop then table.insert(ops, attachedop) end

            local res = ctx.syscall({
                op = "State",
                ops = ops
            })
            assert(res.op == "State", "internal error: State syscall did not return State op")
            return nil
        end)
    end

    local function unsubscribe(event: string, system: string) 
        eventrefmu.call(function() 
            local res = ctx.syscall({
                op = "State",
                ops = {
                    {
                        op = "UnsubscribeEvent",
                        event = event,
                        system = system
                    }
                }
            })
            assert(res.op == "State", "internal error: State syscall did not return State op")
            return nil
        end)
    end

    local function _runEventResult(dispatchable: Primitives.Dispatchable, rootctx: Primitives.TemplateContext, event: Primitives.Event): Primitives.DispatchResult
        local ok, res = xpcall(dispatchable.runEvent, function(e) return debug.traceback(tostring(e), 2) end, rootctx, event)

        if ok then
            return {type = "ok", id = dispatchable.id, value = res}
        else
            return {type = "err", id = dispatchable.id, value = res}
        end
    end

    local function dispatch(event: Primitives.Event): {Primitives.DispatchResult}
        if attachedisolatescount == 0 then return {} end -- unlikely but could happen
        ctx.stream.publish("debug", "Dispatching event " .. event.name)
        local results = table.create<<Primitives.DispatchResult>>(attachedisolatescount)
        for _, isol in attachedisolates do 
            local ret = _runEventResult(isol, ctx, event)
            if ret.type == "err" then
                ctx.stream.publish("debug", { message = ret.value, source = isol.id })
            end
            table.insert(results, ret)
        end
        
        return results
    end

    local function dispatchSingle(event: Primitives.Event, id: string): any
        local isol = attachedisolates[id]
        if not isol then return nil end
        return isol.runEvent(ctx, event)
    end

    local function attach(isolate: Primitives.Dispatchable) 
        if isolate.id == "builtins" then error("cannot modify protected isolate `builtins`") end
        if attachedisolates[isolate.id] then 
            attachedisolates[isolate.id] = isolate
        else
            attachedisolates[isolate.id] = isolate 
            attachedisolatescount += 1
        end
    end

    local function detach(id: string) 
        if id == "builtins" then error("cannot modify protected isolate `builtins`") end
        if not attachedisolates[id] then return end -- no op if not already attached
        attachedisolates[id] = nil 
        attachedisolatescount -= 1
    end

    return table.freeze{
        isSubscribed = isSubscribed,
        subscribe = subscribe,
        unsubscribe = unsubscribe,
        dispatch = dispatch,
        dispatchSingle = dispatchSingle,
        attach = attach,
        detach = detach
    }
end

--- @noyield
local function StreamManager(ctx: Primitives.TemplateContext): Primitives.StreamManager
    local connections: {[number]: string} = {}
    local subscriptions: {[string]: {[number]: boolean}} = {}
    local stream_tx = ctx.btd().stream_tx
    local stream_rx = ctx.btd().stream_rx

    local function close(id: number)
        stream_tx:send({ op = "Close", id = id })
    end
    
    local function publish(topic: string, msg: any)
        local subs = subscriptions[topic]
        if not subs then return end
        local conn_ids = {}
        local payload: Primitives.StreamClientPayload = { t = "msg" :: "msg", n = topic, msg = msg }
        for id, _ in subs do
            table.insert(conn_ids, id)
        end
        stream_tx:pubsync(conn_ids, payload)
    end
    
    local function start_background_task()
        task.spawn(function()
            local ok, _err = pcall(function()
                while true do
                    local msg = stream_rx:recv()
                    if not msg then break end
                    
                    if msg.type == "NewConn" then
                        connections[msg.id] = msg.user_id
                    elseif msg.type == "CloseConn" then
                        connections[msg.id] = nil
                        for _, subs in subscriptions do
                            subs[msg.id] = nil
                        end
                    elseif msg.type == "Msg" then
                        local payload = msg.msg :: Primitives.StreamClientPayload
                        if type(payload) == "table" and type(payload.n) == "string" then
                            if payload.t == "sub" then
                                local topic = payload.n
                                if not subscriptions[topic] then subscriptions[topic] = {} end
                                subscriptions[topic][msg.id] = true
                            elseif payload.t == "unsub" then
                                local topic = payload.n
                                if subscriptions[topic] then subscriptions[topic][msg.id] = nil end
                            elseif payload.t == "msg" and payload.msg ~= nil then
                                -- Message routing can be added here if needed in the future
                            else
                                close(msg.id)
                            end
                        else
                            close(msg.id)
                        end
                    end
                end
            end)
            
            if not ok then
                task.wait(1)
                start_background_task()
            end
        end)
    end
    start_background_task()
    
    return table.freeze({
        publish = publish
    })
end

export type SetupData = {
    read ctx: Primitives.TemplateContext,
    read updatetenantstate: (newts: runtime.TenantState) -> ()
}

--- @noyield
---
--- Creates a new TemplateLoopManager instance at initialization. Using this outside of initialization may fail or lead to hard-to-debug errors
---
--- Note: Do not use this outside of the initial loop creation
local function newinitloop(syscall: runtime.RawSyscall, ts: runtime.TenantState, btd: runtime.BaseTenantData): SetupData
    local tenantstate = ts
    local function dosyscall(args: runtime.SyscallArgs): runtime.SyscallRet
        local res = syscall:async(args)
        if res.op == "State" and res.new_tenant_state then tenantstate = res.new_tenant_state end
        return res
    end
    local function dobtd() return btd end
    local function dotenantstate() return tenantstate end
    local eventmanager: Primitives.EventManager
    local discord: Discord.DiscordClient
    local streammanager: Primitives.StreamManager
    local ctx: Primitives.TemplateContext = {
        syscall = dosyscall,
        btd = dobtd,
        tenantstate = dotenantstate,
        loop = eventmanager,
        discord = discord,
        stream = streammanager,
    }
    eventmanager = EventManager(ctx)
    discord = Discord(ctx)
    streammanager = StreamManager(ctx)

    local ctxany = ctx :: any
    ctxany.discord = discord
    ctxany.loop = eventmanager
    ctxany.stream = streammanager
    
    return table.freeze{ctx=ctx, updatetenantstate = function(newts: runtime.TenantState) tenantstate = newts end}    
end

return {
    newinitloop = newinitloop,
    EventManager = EventManager,
    StreamManager = StreamManager,
    Discord = Discord
}