--[[
    A Blob is a special structure that is owned by Rust
    and can be used to e.g. avoid copying between Lua and Rust

    When a Blob is passed into a Rust, its contents may be moved
    to Rust leaving a empty Blob. `clone` can be used to avoid this. When this
    will happen is undefined.

    This core primitive is not available in WASM contexts yet
]]
export type Blob = buffer


return {}