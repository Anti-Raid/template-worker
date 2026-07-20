local lazy = require("@antiraid-core/lazy")

--- A multi option is either T (Some(Some(T)), a empty table (Some(None)), or nil (None)
export type MultiOption<T> = T | {} | nil
export type i64 = number | integer

--- Opaque object that cannot be read by Luau
export type Opaque = { __phantom: "opaque" }

--- A MemoryVfs type that can be made into a real Vfs using Vfs.newoverlay
export type MemoryVfs = {
    data: {[string]: string},
    __phantom: "MemoryVfs"
}

export type Vfs = {
    --- Given a set of Vfs's (which may either be a existing vfs provided by rust or a lazy string map created using `newlazystringmap` or stored within key-value API), creates an overlay Vfs
    newoverlay: (vfss: {Vfs | MemoryVfs | Opaque}) -> Vfs,
    --- Creates a require function that can be used to require files from the Vfs
    createrequirefunction: (self: Vfs, cacheprefix: string, global_table: {[any]: any}) -> (string) -> ...any,
}

--- Generates a random string of length `length`
local function randstring(length: number): string
    --- A crypto-secure one is implemented in the AntiRaid runtime
    local chars = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789"
    local result = ""
    for i = 1, length do
        local index = math.random(1, #chars)
        result = result .. string.sub(chars, index, index)
    end
    return result
end

--- Given a value, returns a lazy string map (Lazy<{ [string]: string }>)
---
--- The result of this can be passed to APIs like Vfs.newoverlay
local function createvfs(value: { [string]: string }): lazy.Lazy<MemoryVfs>
    error("Implemented internally in AntiRaid runtime!")
end

--- Creates a new global proxy table that forwards reads to the main Luau global table
--- and writes to a separate table.
---
--- Only for advanced use-cases
local function createglobalproxy(): {[any]: any}
    error("Implemented internally in AntiRaid runtime!")
end

--- Note: this is a proxy type and as such, only static methods can be called on it
---
--- Only for advanced use-cases
local Vfs = {} :: Vfs

--- Pretty prints `...` to a string
local function fmtpretty(...): string
    error("Implemented internally in AntiRaid runtime!")
end

return {
    randstring = randstring,
    createglobalproxy = createglobalproxy,
    createvfs = createvfs,
    Vfs = Vfs,
    fmtpretty = fmtpretty
}