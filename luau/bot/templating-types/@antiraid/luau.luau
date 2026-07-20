export type Chunk = {
    --- Sets the environment of the chunk (_G).
    environment: {[any]: any}?,
    
    --- Sets the optimization level of the chunk.
    optimization_level: number?,
    
    --- Text code to be evaluated. Bytecode evaluation is not allowed due to
    --- security reasons.
    code: string,
    
    ---  The name of the chunk, used for debugging purposes.
    chunk_name: string?,

    --- Takes in args and returns the returned values from the ``code`` being evaluated. This will run the code in main thread / coroutine.running() == nil
    call: (self: Chunk, args: any) -> any,

    --- @yields
    ---
    --- Takes in args and returns the returned values from the ``code`` being evaluated.
    ---
    --- This runs the code asynchronously within a coroutine, allowing it to call
    --- yielding functions
    call_async: (self: Chunk, args: any) -> any,
}

--- Loads a Luau chunk.
local function load(code: string): Chunk
    error("Implemented internally in AntiRaid runtime!")
end

--- Formats a set of values to a string
local function format(...: any): string
    error("Implemented internally in AntiRaid runtime!")
end

return {
    load = load,
    format = format,
}