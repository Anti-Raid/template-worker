--!strict
local Primitives = require"@antiraid-core/primitives"
local blob = require("@antiraid-core/blob")
local datetime = require"@antiraid/datetime"
local typesext = require"@antiraid/typesext"

local KEY_RANDSTRING_LENGTH = 8

--- KeyRecord represents a key-value of the record.
---@class KvRecord
export type KeyRecord<T> = {
    --- The key of the record.
    read key: string,
    --- The value of the record. This can be any type, depending on what was stored.
    read value: T,
    --- The timestamp when the record was created
    read createdat: datetime.DateTime,
    --- The timestamp when the record was last updated at
    read lastupdatedat: datetime.DateTime,
}

--- KeyRecord enriched with blob data
export type KeyRecordWithBlob<T> = KeyRecord<T> & {
    --- The blob associated with the record, if any. This is used for storing small binary data that can be downloadable
    read blob: blob.Blob?,
}

export type SignedUrl = {
    --- The signed URL that can be used to access the blob. This URL will only be valid for a limited time, as specified by the expiry field.
    read url: string,
    --- The number of seconds until the URL expires. After this time, the URL will no longer work.
    read expiry: number,
}

--- A key manager provides methods to add, remove and check keys with builtin caching
---
--- Note that the associated data present should be small in size as all keys are loaded into memory.
export type KeyManager<T> = {
    --- Makes a new key, returning the generated key
    read add: (data: T, key: string?, blob: blob.BlobTaker?) -> string,

    --- Updates the data associated with a key. Does not affect anything but the data.
    read updatedata: (key: string, data: T, blob: blob.BlobTaker?) -> (),

    --- Removes a key
    read remove: (key: string) -> (),

    --- Checks if a key exists and is not expired
    read exists: (key: string) -> boolean,

    --- Tries to synchronize key expiries from tenant state
    --- @yields
    read sync: () -> (),

    --- Lists all keys with their data
    ---
    --- Note that the data returned is not cloned, so modifying it may modify actual stored data. This is for performance reasons since the data can be large, 
    --- but be aware of this when using the list method.
    read list: () -> {[string]: KeyRecord<T>},

    --- Gets a specific key's data
    read get: (key: string) -> KeyRecord<T>?,

    --- Fetches the loaded kv blob associated with a key. This is used for storing small binary data that can be downloadable
    read getwithblob: (key: string) -> KeyRecordWithBlob<T>?,

    --- Generates a URL for downloading the blob associated with a key. This URL will not work if there is no blob or if the key does not exist.
    ---
    --- Additionally, the returned url will only be valid for `expiry` seconds, and will not work after that time.
    read signbloburl: (key: string) -> SignedUrl,

    --- Returns the number of keys currently stored (approximate)
    read count: () -> number,
}

--- A key manager
---
--- A key manager can be used to handle data with caching. Similar to KeyExpiryManager but without the expiry functionality
local function KeyManager<T>(
    ctx: Primitives.TemplateContext, 
    basescope: string,
    oninit: ((records: {[string]: KeyRecord<T>}, km: KeyManager<T>) -> any)?
): KeyManager<T>
    local kexs = {} :: {[string]: KeyRecord<T>}
    local count = 0

    local function remove(key: string) 
        if not kexs[key] then return end -- Remove associated key + data
        -- Remove key
        ctx.syscall({op="State", ops={{ op = "KvDelete", key = key, scope = basescope }}})
        kexs[key] = nil
        count = count - 1
        if next(kexs) == nil then 
            ctx.loop.unsubscribe("@keym", basescope) -- No more keys, unsubscribe
        end
    end

    local function exists(id: string): boolean 
        return kexs[id] ~= nil
    end

    local function _createkey(): string 
        local key = typesext.randstring(KEY_RANDSTRING_LENGTH)
        while true do 
            if exists(key) then
                key = typesext.randstring(KEY_RANDSTRING_LENGTH)
            else
                break
            end
        end
        return key
    end

    local function add(data: T, ckey: string?, blob: blob.BlobTaker?)
        local key = ckey or _createkey()
        assert(not exists(key), `Generated key \`{key}\` already exists`)

        -- Record in kv store and subscribe to event via attachedop
        ctx.loop.subscribe("@keym", basescope, { op = "KvSet", key = key, scope = basescope, value = data, blob = blob })
        local now = datetime.UTC:now()
        kexs[key] = table.freeze{
            key = key,
            createdat = now,
            lastupdatedat = now,
            value = data,
        }
        count = count + 1

        return key
    end

    local function updatedata(key: string, data: T, blob: blob.BlobTaker?)
        if not exists(key) then
            return
        end

        -- Update in kv store, note that the delayqueue only stores key so we don't need to update it
        local record = kexs[key]
        ctx.syscall({op="State", ops={{ op = "KvSet", key = key, scope = basescope, value = data, blob = blob }}})
        kexs[key] = table.freeze{
            key = key,
            createdat = record.createdat,
            lastupdatedat = datetime.UTC:now(),
            value = data,
        }
    end

    local function list(): {[string]: KeyRecord<T>}
        return kexs
    end

    local function get(key: string): KeyRecord<T>?
        local record = kexs[key]
        if not record then return nil end
        return record
    end

    --- getwithblob
    local function getwithblob(key: string): KeyRecordWithBlob<T>?
        local res = ctx.syscall({op="State", ops={{ op = "KvGetWithBlob", key = key, scope = basescope }}})
        assert(res.op == "State")

        local kvres = res.res[1]
        assert(kvres.op == "KvWithBlob")

        return table.freeze{
            key = kvres.key,
            createdat = kvres.created_at,
            lastupdatedat = kvres.last_updated_at,
            value = kvres.value,
            blob = kvres.blob,
        }
    end

    --- signbloburl
    local function signbloburl(key: string): SignedUrl
        local res = ctx.syscall({op="State", ops={{ op = "KvSignUrl", key = key, scope = basescope }}})
        assert(res.op == "State")

        local signres = res.res[1]
        assert(signres.op == "KvSignUrl")

        return table.freeze{
            url = signres.url,
            expiry = signres.expiry,
        }
    end

    local self = {} :: KeyManager<T>

    --- Synchronize key expiries from KV store
    local function sync() 
        -- Fetch existing key expiries from kv
        -- Syncing keys from tenant state
        local kexdata = ctx.syscall({op="State", ops={{ op = "KvFind", query = "%%", scope = basescope}}})
        assert(kexdata.op == "State")
        local newkexs: {[string]: KeyRecord<T>} = {}
        local newcount = 0
        for _, item in kexdata.res do
            if item.op ~= "Kv" then continue end
            local record = item.value :: T
            newkexs[item.key] = table.freeze{
                key = item.key,
                value = record,
                createdat = item.created_at,
                lastupdatedat = item.last_updated_at
            }
            newcount = newcount + 1
        end
        kexs = newkexs -- Replace with new data
        count = newcount
        -- Synced keys

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
    end

    local function countkeys(): number 
        return count
    end

    local _self: KeyManager<T> = table.freeze{
        add = add,
        remove = remove,
        exists = exists,
        sync = sync,
        list = list,
        get = get,
        updatedata = updatedata,
        count = countkeys,
        getwithblob = getwithblob,
        signbloburl = signbloburl
    }
    self = _self -- For luau type reasons

    -- Initially sync if we're subscribed to OnStartup explicitly.
    if ctx.loop.isSubscribed("@keym", basescope) then
        task.spawn(sync)
    end

    return _self
end

return KeyManager