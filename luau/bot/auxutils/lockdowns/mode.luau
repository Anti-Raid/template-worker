local discord = require("@discord-types/apiTypes")
local datetime = require("@antiraid/datetime")
local handle = require("./handle")
local set = require"@antiraid-ext/set"
local Primitives = require"@antiraid-core/primitives"

export type Lockdown = {
    id: string,
    reason: string,
    type: LockdownMode,
    data: any, -- Data specific to the lockdown mode
    created_at: datetime.DateTime -- Creation time of the lockdown
}

--- Base lockdown mode data
export type BaseLockdownModeData = {
    guild: discord.GuildObject,
    channels: {discord.ChannelObject},
    critical_roles: set.Set<discord.Snowflake>,
    lockdowns: {Lockdown},
    ctx: Primitives.TemplateContext
}

--- Extended lockdown mode data
export type ExtLockdownModeData = BaseLockdownModeData & {
    all_handles: handle.LockdownModeHandles,
    lockdown: Lockdown,
}

export type LockdownMode = {
    --- Returns the string form of the lockdown mode
    string_form: () -> string,

    --- The specificity of the lockdown mode. More specific lockdowns should have higher specificity
    ---
    --- The specificity is used to determine which lockdowns should be applied/reverted in the event of multiple lockdowns
    --- handling the same roles/channels
    specificity: () -> number,

    --- Tests that a lockdown mode can in fact work
    test: (data: BaseLockdownModeData) -> string?,

    --- Try to automatically fix any issues that may prevent the lockdown mode from being applied
    autofix: (data: BaseLockdownModeData) -> (),

    --- Sets up the lockdown mode, returning any data to be stored with the lockdown
    setup: (data: BaseLockdownModeData) -> any,

    --- Returns the sharable lockdown data
    shareable: (lockdown: Lockdown) -> handle.LockdownSharableData,

    --- Creates a new lockdown
    create: (data: ExtLockdownModeData) -> (),

    --- Reverts a lockdown
    revert: (data: ExtLockdownModeData) -> (),

    --- Returns the handle for the lockdown
    handle: (data: BaseLockdownModeData, lockdown: Lockdown) -> handle.LockdownModeHandle,
}

local DummyLockdownMode: LockdownMode = {
    string_form = function() return "dummy" end,
    specificity = function() return -math.huge end,
    test = function(data): string? return "Cannot apply a test lockdown" end,
    autofix = function(data): () error("Cannot apply a test lockdown") end,
    setup = function(data) error("Cannot apply a test lockdown") end,
    shareable = function(lockdown): any error("Cannot apply a test lockdown") end,
    create = function(data): () error("Cannot apply a test lockdown") end,
    revert = function(data): () error("Cannot apply a test lockdown") end,
    handle = function(data, lockdown): any error("Cannot apply a test lockdown") end,
}

export type RegistryCall = (stringform: string) -> LockdownMode?;
export type LockdownModeRegistry = {RegistryCall}

local function getLockdownMode(reg: LockdownModeRegistry, stringform: string): LockdownMode?
    for _, func in reg do
        local mode = func(stringform)
        if mode then
            return mode
        end
    end
    return nil
end

return {
    getLockdownMode = getLockdownMode,
    DummyLockdownMode = DummyLockdownMode
}