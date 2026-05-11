--!strict

-- See https://github.com/Anti-Raid/khronos/blob/master/crates/ext/src/db.rs for the canonical source of truth for these types.

--- A wrapper around a sqlx PgPool that provides methods to execute queries and fetch results, with values mapped through the DbValueMapper
export type Db = {
    --- Returns if we support the given type for casting
    supports: (self: Db, type: string) -> boolean,
    --- Casts a Lua value into an OpaqueValue of the specified type.
    cast: (self: Db, value: any, type: string) -> DbValue,
    --- Executes a query and returns number of affected rows
    execute: (self: Db, sql: string, params: {DbValue}) -> number,
    --- Executes a query and returns all results
    fetchall: (self: Db, sql: string, params: {DbValue}) -> {PgRow},
    --- Begins a transaction and returns a DbTx object that can be used to execute queries within the transaction
    begin: (self: Db) -> DbTx,
}

--- A wrapper around a sqlx Transaction that provides methods to execute queries and fetch results, with values mapped through the DbValueMapper
export type DbTx = {
    --- Executes a query and returns number of affected rows
    execute: (self: DbTx, sql: string, params: {DbValue}) -> number,
    --- Executes a query and returns all results
    fetchall: (self: DbTx, sql: string, params: {DbValue}) -> {PgRow},
    --- Commits the transaction
    commit: (self: DbTx) -> (),
    --- Rolls back the transaction
    rollback: (self: DbTx) -> (),
}

export type PgRow = {
    --- Gets the value of a column by index (1-based) and type. Does not immediately convert the value to a Lua type, but instead returns a Value object that can be converted on demand.
    get: (self: PgRow, index: number, type: string) -> DbValue,
}

export type DbValue = {
    --- Gets the underlying type 
    type: (self: DbValue) -> string,
    --- Takes out the underlying value
    take: (self: DbValue) -> any,
}

export type Id = {
    type: "Guild",
    id: string,
} | {
    type: "User",
    id: string,
}

return {}