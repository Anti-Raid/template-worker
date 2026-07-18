--!strict
local Primitives = require"@antiraid-core/primitives"
local id = require "./id"

export type WorkerLike = typeof(setmetatable({} :: {
    ID: (self: WorkerLike) -> number,
    Kill: (self: WorkerLike) -> (),
    RunScript: (self: WorkerLike, id: id.LuaId, name: string, code: string, event: Primitives.Event) -> (any),
    DispatchEvent: (self: WorkerLike, id: id.LuaId, event: Primitives.Event) -> (any),
    DispatchEventNoWait: (self: WorkerLike, id: id.LuaId, event: Primitives.Event) -> (),
    DropTenant: (self: WorkerLike, id: id.LuaId) -> (),
}, {} :: {
    --- Returns the number of workers within the worker-like if a pool. Returns 0 if not a pool.
    __len: (self: WorkerLike) -> number,
}))

return {}