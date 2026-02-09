--!strict
export type Db = {
    --- Creates a new index map to convert a column of a row into a value
    indexmap: (type: string) -> DbIndexMapper,
    --- Casts a Lua value into an OpaqueValue of the specified type.
    cast: (value: any, type: string) -> OpaqueValue,
    --- Executes a query and returns all results
    fetchall: (sql: string, params: {any}) -> {PgRow},
}

export type DbIndexMapper = {
    __phantomT: "DbIndexMapper",
}

export type PgRow = {
    --- Gets the value of a column by index (1-based) and type. Does not immediately convert the value to a Lua type, but instead returns a Value object that can be converted on demand.
    get: (index: number, map: DbIndexMapper) -> OpaqueValue,
}

export type OpaqueValue = {
    --- Gets the underlying type 
    type: () -> string,
    --- Gets the underlying value
    get: () -> any,
    --- Clones the opaquevalue
    clone: () -> OpaqueValue,
}

export type Id = {
    type: "Guild",
    id: string,
} | {
    type: "User",
    id: string,
}
