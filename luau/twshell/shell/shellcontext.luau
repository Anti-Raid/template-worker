local kvgod = require"../common/kvgod"
local msyscall = require"@antiraid-core/msyscall"

export type ShellContext = {
    __phantom: "ShellContext",
    --- MSyscall
    MSyscall: (self: ShellContext, args: msyscall.MSyscallArgs) -> kvgod.KvGod,
    --- Gets input from the user
    GetInput: (self: ShellContext, prompt: string) -> string?,
    --- Logs values to the console
    Log: (self: ShellContext, ...any) -> (),
}

return {}