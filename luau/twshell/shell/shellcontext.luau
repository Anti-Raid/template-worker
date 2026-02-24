local kvgod = require"../common/kvgod"
export type ShellContext = {
    __phantom: "ShellContext",
    --- Create a new KvGod instance for key-value operations
    CreateKvGod: (self: ShellContext) -> kvgod.KvGod,
    --- Register commands globally
    RegisterGlobalCommands: (self: ShellContext, commands: {any}) -> (),
    --- Gets input from the user
    GetInput: (self: ShellContext, prompt: string) -> string?,
    --- Logs values to the console
    Log: (self: ShellContext, ...any) -> (),
}

return {}