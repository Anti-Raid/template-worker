local Primitives = require "@antiraid-core/primitives"
local KeyManager = require"@antiraid-ext/keymanager"

--- Stores common/persistent data required for lockdowns
export type LockdownSettings = {
    member_roles: {string},
    --- Whether or not lockdown tests should pass or not
    no_apply_test: boolean,
}

export type LockdownSettingsManager = {
    get: () -> LockdownSettings | nil,
    set: (settings: LockdownSettings) -> (),
    delete: () -> (),
    default: () -> LockdownSettings,
}

--- A settings manager for the lockdown settings.
local function LockdownSettingsManager(ctx: Primitives.TemplateContext): LockdownSettingsManager
    local self = {}
    local settings = KeyManager<<LockdownSettings>>(ctx, "builtins.lockdown_settings")

    local function get(): LockdownSettings | nil
        local settings = settings.get("lockdown_settings")
        if settings then return settings.value else return nil end
    end

    local function set(setting: LockdownSettings)
        settings.add(setting, "lockdown_settings")
    end

    local function delete()
        settings.remove("lockdown_settings")
    end

    local function default(): LockdownSettings
        return {
            member_roles = {},
            no_apply_test = false,
        }
    end

    self.get = get
    self.set = set
    self.delete = delete
    self.default = default

    return self
end

return LockdownSettingsManager