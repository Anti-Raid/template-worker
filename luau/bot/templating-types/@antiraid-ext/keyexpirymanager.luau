local Primitives = require"@antiraid-core/primitives"
local channel = require"@antiraid/channel"
local datetime = require"@antiraid/datetime"
local typesext = require"@antiraid/typesext"

local KEYEXPIRY_RANDSTRING_LENGTH = 8

--- ExpiringKeyRecord represents a key-value of the record.
---@class KvRecord
export type ExpiringKeyRecord<T> = {
    --- The key of the record.
    read key: string,
    --- The value of the record. This can be any type, depending on what was stored.
    read value: T,
    --- The timestamp when the record was created
    read createdat: datetime.DateTime,
    --- The timestamp when the record will expire, 
    read expiresat: datetime.DateTime,
}

--- A key expiry manager provides methods to add, remove and check keys with expiries
---
--- When a key expires, a key expiry event is dispatched to all templates.
---
--- Note that the associated data present should be small in size as all keys are loaded into memory.
export type KeyExpiryManager<T> = {
    --- Makes a new key expiry, returning the generated key
    ---
    --- When the returned key expires after `expiresin`, a `KeyExpiry` event is dispatched to all templates with the key
    --- as part of the event data.
    ---
    --- Any data to be associated with the key expiry should be stored separately using the returned key (e.g. in the key-value store).
    read add: (expiresin: datetime.TimeDelta, data: T, key: string?) -> string,

    --- Same as ``add`` but takes in a DateTime expiresat instead of a TimeDelta
    read addat: (expiresat: datetime.DateTime, data: T, key: string?) -> string,

    --- Same as ``add`` but takes in a expiresin as seconds instead of a TimeDelta
    read addwithsecs: (expiresin: number, data: T, key: string?) -> string,

    --- Updates the data associated with a kex expiry. Does not affect anything but the data returned in the event when the key expires.
    read updatedata: (key: string, data: T) -> (),

    --- Removes a key
    read remove: (key: string) -> (),

    --- Checks if a key exists and is not expired
    read exists: (key: string) -> boolean,

    --- Tries to synchronize key expiries from tenant state


    --- Lists all keys with their data
    ---
    --- Note that the data returned is not cloned, so modifying it may modify actual stored data. This is for performance reasons since the data can be large, 
    --- but be aware of this when using the list method.
    read list: () -> {[string]: ExpiringKeyRecord<T>},

    --- Gets a specific key expiry's data
    read get: (key: string) -> ExpiringKeyRecord<T>?,

    --- Returns the number of keys currently stored (approximate)
    read count: () -> number,
}

--- A key expiry manager
---
--- A key expiry manager can be used to schedule events to be dispatched at a later time with associated data.
local function KeyExpiryManager<T>(
    ctx: Primitives.TemplateContext, 
    scope: string, 
    callback: (record: ExpiringKeyRecord<T>) -> any, 
    oninit: ((records: {[string]: ExpiringKeyRecord<T>}, kexm: KeyExpiryManager<T>) -> any)?
): KeyExpiryManager<T>
    local systemscope = "__kexs"..scope -- System scope where kex's are stored

    local kexs = {} :: {[string]: ExpiringKeyRecord<T>}
    local count = 0

    local handles: {[string]: channel.KeyHandle<string>} = {}
    local delayqueue = channel.DelayChannel() :: channel.DelayChannel<string> -- Delayqueue stores a queue of all the kvs we need to dispatch
    local th: thread? = nil

    local function remove(key: string) 
        if not kexs[key] then return end
        
        -- First, drop the key, this may error in which we halt immediately
        ctx.syscall({op="State", ops={{ op = "KvDelete", key = key, scope = systemscope }}})
        local handle = handles[key]
        if handle then 
            handle:cancel() -- Cancel the handle
            handles[key] = nil
        end
        kexs[key] = nil
        count = count - 1
        if next(kexs) == nil and #delayqueue == 0 then 
            ctx.loop.unsubscribe("OnStartup", systemscope) -- No more key expiries, unsubscribe
        end
    end

    --- Calls the callback with the expired key and its associated data, then removes the key expiry
    local function _callcallback(key: string) 
        -- Dispatch event to templates

        -- Fetch associated data if any
        local kex = kexs[key]

        local ok, res = xpcall(callback, debug.traceback :: any, kex)
        if not ok then 
            ctx.stream.publish("error", res)
        end

        -- Remove the key expiry only after dispatching
        handles[key] = nil
        remove(key)
    end

    --- The main loop that waits for key expiries
    local function _loop(): any 
        while true do 
            local item = delayqueue:next()

            -- Dispatch events in a seperate thread to avoid blocking the loop
            task.spawn(_callcallback, item)
        end
    end

    --- Loop wrapper to catch crashes and respawn the loop if needed
    local function _loopwhile() 
        while true do
                local success, err = pcall(_loop)
                if not success then
                    ctx.stream.publish("error", err)
                    task.wait(1) -- Prevent CPU spin
                end
            end
    end

    --- Helper method to ensure the loop is running
    local function _ensureLoopRunning()
        if not th or coroutine.status(th) == "dead" then th = task.spawn(_loopwhile) end
    end

    local function exists(id: string): boolean 
        return kexs[id] ~= nil and handles[id] ~= nil
    end

    local function _createkey(): string 
        local key = typesext.randstring(KEYEXPIRY_RANDSTRING_LENGTH)
        while true do 
            if exists(key) then
                key = typesext.randstring(KEYEXPIRY_RANDSTRING_LENGTH)
            else
                break
            end
        end
        return key
    end

    --- Returns the duration to wait until a key expires, adding a random delay to avoid thundering herd if the expiry time has already passed
    local function _getduration(expiry: datetime.DateTime): datetime.TimeDelta
        local expiresat = expiry
        if expiry <= datetime.UTC:now() then 
            -- Already expired, add with a random delay > 20 seconds to avoid thundering herd
            expiresat = datetime.UTC:now() + datetime.timedelta_seconds(20 + math.random(5, 20))
        end
        return expiresat:duration_since(datetime.UTC:now())
    end

    local function addat(expiresat: datetime.DateTime, data: T, ckey: string?)
        local key = ckey or _createkey()
        assert(not exists(key), `Generated KeyExpiry key \`{key}\` already exists`)
        assert(expiresat.timezone == datetime.UTC, "expiresat must be in UTC timezone, currently: "..tostring(expiresat.timezone))

        -- Record in kv store and subscribe to event via attachedop
        ctx.loop.subscribe(
            "OnStartup", 
            systemscope, 
            { 
                op = "KvSet", 
                key = key, 
                scope = systemscope, 
                value = {
                    expiresat = expiresat,
                    data = data,
                }    
            }
        )
        kexs[key] = table.freeze{
            key = key,
            createdat = datetime.UTC:now(),
            expiresat = expiresat,
            value = data,
        }
        count = count + 1
        handles[key] = delayqueue:add(key, _getduration(expiresat))
        _ensureLoopRunning()

        return key
    end

    -- Just calls addat with a TimeDelta
    local function add(expiresin: datetime.TimeDelta, data: T, key: string?)
        local expiresat = (datetime.UTC:now() + expiresin)
        return addat(expiresat, data, key)
    end

    local function addwithsecs(expiresin: number, data: T, key: string?)
        local expiresat = (datetime.UTC:now() + datetime.timedelta_seconds(expiresin))
        return addat(expiresat, data, key)
    end

    local function updatedata(key: string, data: T)
        if not exists(key) then 
            return
        end

        -- Update in kv store, note that the delayqueue only stores key so we don't need to update it
        local record = kexs[key]
        ctx.syscall({op="State", ops={{
            op = "KvSet",
            key = key,
            scope = systemscope,
            value = {
                expiresat = record.expiresat,
                data = data,
            }
        }}})
        kexs[key] = table.freeze{
            key = key,
            createdat = record.createdat,
            expiresat = record.expiresat,
            value = data,
        }
    end

    local function list(): {[string]: ExpiringKeyRecord<T>}
        return kexs
    end

    local function get(key: string): ExpiringKeyRecord<T>?
        local record = kexs[key]
        if not record then return nil end
        return record
    end

    local self = {} :: KeyExpiryManager<T>

    --- Synchronize key expiries from KV store
    local function sync() 
        if #delayqueue > 0 then delayqueue:clear() end -- Clear existing delayqueue entries
        -- Fetch existing key expiries from kv
        local kexdata = ctx.syscall({op="State", ops = {{ op = "KvFind", query = "%%", scope = systemscope}}})
        assert(kexdata.op == "State")
        local newkexs: {[string]: ExpiringKeyRecord<T>} = {}
        local newcount = 0
        for _, item in kexdata.res do
            if item.op ~= "Kv" then continue end
            local record = item.value :: {
                expiresat: datetime.DateTime,
                data: T,
            }
            newkexs[item.key] = table.freeze{
                key = item.key,
                value = record.data,
                createdat = item.created_at,
                expiresat = record.expiresat,
            }
            newcount = newcount + 1
        end
        kexs = newkexs -- Replace with new data
        count = newcount

        -- Call oninit callback if provided
        if oninit then 
            task.spawn(function()
                if not oninit then return end -- needed for luau new type solver
                local ok, res = xpcall(oninit, debug.traceback :: any, kexs, self)
                if not ok then 
                    ctx.stream.publish("error", res)
                end
            end)
        end

        -- Setup delayqueue
        _ensureLoopRunning()
        local count = 0
        for key, data in kexs do
            count += 1
            if count % 200 == 0 then 
                task.wait(0.1) -- Yield to avoid blocking too long
            end
            handles[key] = delayqueue:add(key, _getduration(data.expiresat))
        end    
    end

    local function countkeys(): number 
        return count
    end

    local _self: KeyExpiryManager<T> = table.freeze{
        add = add,
        addwithsecs = addwithsecs,
        addat = addat,
        remove = remove,
        exists = exists,
        sync = sync,
        list = list,
        get = get,
        updatedata = updatedata,
        count = countkeys,
    }
    self = _self -- For luau type reasons

    -- Initially sync if we're subscribed to OnStartup explicitly.
    if ctx.loop.isSubscribed("OnStartup", systemscope) then
        task.spawn(sync)
    end

    return _self
end

return KeyExpiryManager