local khronosvalue = require "@antiraid-core/khronosvalue"
local datetime = require "@antiraid/datetime"
local id = require "./id"

--- KvRecord represents a key-value record with metadata.
---@class KvRecord
export type KvRecord = {
    --- The ID of the record
    id: string,
    --- The key of the record.
    key: string,
    --- The value of the record. This can be any type, depending on what was stored.
    value: khronosvalue.KhronosValue,
    --- Indicates whether the record exists in the key-value store.
    exists: true,
    --- The scopes the key has
    scopes: {string},
    --- The timestamp when the record was created, in ISO 8601 format.
    created_at: datetime.DateTime,
    --- The timestamp when the record was last updated, in ISO 8601 format.
    last_updated_at: datetime.DateTime,
} | {
    --- The key of the record.
    key: string,
    --- Indicates whether the record exists in the key-value store.
    exists: false,
}

--- A list of KvRecords
export type KvRecordList = {KvRecord}

--- KvExecutor allows templates to get, store and find persistent data within a server.
---@class KvExecutor
export type KvGod = {
    --- @yields
    ---
    --- Returns a list of all key-value scopes/
    --- This is only guaranteed to return scopes that actually have at least one key inside it.
    ---
    --- Needs the `kv.meta:list_scopes` capability to use
    list_scopes: (self: KvGod, id: id.LuaId) -> {string},

    --- @yields
    ---
    --- Finds matching records in this key-value scope.
    ---
    --- Within query, % matches zero or more characters; _ matches a single character. To search anywhere in a string, surround {KEY} with %, e.g. %{KEY}%.
    --- %% (match all keys) has a special fast path for performance purposes and should be preferred where possible.
    ---
    --- @param query string The query to search for.
    --- @return {KvRecord} The records.
    find: (self: KvGod, id: id.LuaId, query: string, scopes: {string}) -> KvRecordList,

    --- @yields
    ---
    --- Gets a value from this key-value scope.
    --- @param key string The key of the record.
    --- @return any The value of the record.
    get: (self: KvGod, id: id.LuaId, key: string, scopes: {string}) -> khronosvalue.KhronosValue,

    --- @yields
    ---
    --- Gets a record from this key-value scope.
    --- @param key string The key of the record.
    --- @return KvRecord The record.
    getrecord: (self: KvGod, id: id.LuaId, key: string, scopes: {string}) -> KvRecord,

    --- @yields
    ---
    --- Sets a record in the key-value store.
    --- @param key string The key of the record.
    --- @param value any The value of the record.
    --- @param scopes {string} The scopes to set the record in.
    set: (self: KvGod, id: id.LuaId, key: string, value: khronosvalue.KhronosValue, scopes: {string}) -> (),

    --- @yields
    ---
    --- Deletes a record from this key-value scope
    --- @param key string The key of the record.
    delete: (self: KvGod, id: id.LuaId, key: string, scopes: {string}) -> nil,
}

return {}