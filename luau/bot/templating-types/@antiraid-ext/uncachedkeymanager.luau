--!strict
local Primitives = require"@antiraid-core/primitives"
local blob = require("@antiraid-core/blob")
local datetime = require"@antiraid/datetime"

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

--- An uncached key manager provides methods to add, remove and check keys with *no* caching
export type UncachedKeyManager<T> = {
    --- Inserts or updates a new key
    read set: (key: string, data: T, blob: blob.BlobTaker?) -> (),

    --- Removes a key
    read remove: (key: string) -> (),

    --- Lists all keys with their data
    ---
    --- If query is passed, will use said query (sql ilike syntax for query) for listing, otherwise, will use `%%` to get all keys
    read list: (query: string?) -> (number, {[string]: KeyRecord<T>}),

    --- Same as list but returns an array
    read listarr: (query: string?) -> {KeyRecord<T>},

    --- Gets a specific key's data
    read get: (key: string) -> KeyRecord<T>?,

    --- Fetches the loaded kv blob associated with a key. This is used for storing small binary data that can be downloadable
    read getwithblob: (key: string) -> KeyRecordWithBlob<T>?,

    --- Generates a URL for downloading the blob associated with a key. This URL will not work if there is no blob or if the key does not exist.
    ---
    --- Additionally, the returned url will only be valid for `expiry` seconds, and will not work after that time.
    read signbloburl: (key: string) -> SignedUrl,
}

--- An uncached key manager
---
--- A uncached key manager can be used to handle data *without* caching (similar to KeyManager but without caching)
---
--- Notes:
--- 1. Unlike KeyManager, it is guaranteed that creating a UncachedKeyManager will *not* yield
local function UncachedKeyManager<T>(ctx: Primitives.TemplateContext, basescope: string): UncachedKeyManager<T>
    local function remove(key: string) 
        ctx.syscall({op="State", ops={{ op = "KvDelete", key = key, scope = basescope }}})
    end

    local function set(key: string, data: T, blob: blob.BlobTaker?)
        ctx.syscall({op="State", ops={{ op = "KvSet", key = key, scope = basescope, value = data, blob = blob }}})
    end

    local function list(query: string?): (number, {[string]: KeyRecord<T>})
        local records = ctx.syscall({op="State", ops={{ op = "KvFind", query = query or "%%", scope = basescope }}})
        assert(records.op == "State")
        local parsed: {[string]: KeyRecord<T>} = {}
        local count = 0
        for _, record in records.res do 
            if record.op ~= "Kv" then continue end
            parsed[record.key] = {
                key = record.key,
                value = record.value,
                createdat = record.created_at,
                lastupdatedat = record.last_updated_at
            }
            count += 1
        end

        return count, parsed
    end

    local function listarr(query: string?): {KeyRecord<T>}
        local records = ctx.syscall({op="State", ops={{ op = "KvFind", query = query or "%%", scope = basescope }}})
        assert(records.op == "State")
        local parsed: {KeyRecord<T>} = {}
        for _, record in records.res do 
            if record.op ~= "Kv" then continue end
            table.insert(parsed, {
                key = record.key,
                value = record.value,
                createdat = record.created_at,
                lastupdatedat = record.last_updated_at
            })
        end

        return parsed
    end

    local function get(key: string): KeyRecord<T>?
        local records = ctx.syscall({op="State", ops={{ op = "KvGet", key = key, scope = basescope }}})
        assert(records.op == "State")
        if #records.res == 0 then return nil end
        local record = records.res[1]
        if record.op ~= "Kv" then return nil end
        return {
            key = record.key,
            value = record.value,
            createdat = record.created_at,
            lastupdatedat = record.last_updated_at
        }
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

    local self: UncachedKeyManager<T> = table.freeze{
        set = set,
        remove = remove,
        list = list,
        listarr = listarr,
        get = get,
        getwithblob = getwithblob,
        signbloburl = signbloburl,
    }

    return self
end

return UncachedKeyManager