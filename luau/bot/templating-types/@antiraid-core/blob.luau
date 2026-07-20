--[[
    A Blob is a special structure that is owned by Rust
    and can be used to e.g. avoid copying between Lua and Rust

    When a Blob is passed into a Rust, its contents may be moved
    to Rust leaving a empty Blob. `clone` can be used to avoid this. When this
    will happen is undefined.

    This core primitive is not available in WASM contexts yet
]]
export type Blob = typeof(setmetatable({} :: {
    --- Converts the Blob to a buffer if there is enough memory available
    tobuffer: (self: Blob) -> buffer,
    --- Drains the Blob following which the Blob will be empty
    drain: (self: Blob) -> nil,
    --- Securely drains/"zeroizes" the Blob, following which the Blob will be empty
    zeroize: (self: Blob) -> nil,
}, {} :: {
    __len: (self: Blob) -> number, -- Returns the length of the blob in bytes
}))

export type BlobTaker = buffer | string | Blob 

return {}