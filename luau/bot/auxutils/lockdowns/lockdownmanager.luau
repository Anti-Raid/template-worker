local Primitives = require "@antiraid-core/primitives"
local mode = require("./mode")
local KeyManager = require"@antiraid-ext/keymanager"
local datetime = require "@antiraid/datetime"
local DEFAULT_REGISTRY = require "./moderegistry".DEFAULT_REGISTRY

export type CreateLockdown = {
    id: string?, -- ID of the lockdown, if not provided a random one will be generated
    reason: string, -- Reason for the lockdown
    type: mode.LockdownMode, -- The type of lockdown mode to use
    data: any, -- Data specific to the lockdown mode
}

export type LockdownManager = {
    --- Returns a list of all lockdowns
    list: () -> {mode.Lockdown},
    --- Returns a lockdown by ID
    get: (id: string) -> mode.Lockdown | nil,
    --- Sets a lockdown, creating it if it does not exist
    ---
    --- Returns the ID of the lockdown
    set: (lockdown: CreateLockdown) -> mode.Lockdown,
    --- Deletes a lockdown by ID
    delete: (id: string) -> (),
}

type LockdownData = {
    reason: string,
    type: string,
    data: any
}

--- A data fetcher for lockdowns.
local function LockdownManager(ctx: Primitives.TemplateContext, reg_val: mode.LockdownModeRegistry?): LockdownManager
    local lockdownkeys = KeyManager<<LockdownData>>(ctx, "builtins.lockdowns")
    local reg = reg_val or DEFAULT_REGISTRY
    local self = {}

    local function _parseLockdown(item: KeyManager.KeyRecord<LockdownData>): mode.Lockdown
        local lockdownId = item.key
        local reason = item.value.reason
        local type_val = item.value.type
        local modev = mode.getLockdownMode(reg, type_val)
        local ldata = item.value.data
        local createdAt = item.createdat

        return {
            id = lockdownId,
            reason = reason,
            type = modev or mode.DummyLockdownMode,
            data = ldata,
            created_at = createdAt
        }
    end

    local function list(): {mode.Lockdown}
        local data = lockdownkeys.list()

        local lockdowns = {}
        for _, item in data do
            local lockdown = _parseLockdown(item)
            table.insert(lockdowns, lockdown)
        end

        return lockdowns
    end

    local function get(id: string): mode.Lockdown | nil
        local item = lockdownkeys.get(id)
        if not item then
            return nil
        end

        return _parseLockdown(item)
    end

    local function set(lockdown: CreateLockdown): mode.Lockdown
        local id = lockdownkeys.add({
            reason = lockdown.reason,
            type = lockdown.type.string_form(),
            data = lockdown.data,
        }, lockdown.id)

        return {
            id = id,
            reason = lockdown.reason,
            type = lockdown.type,
            data = lockdown.data,
            created_at = datetime.UTC:now(), -- For now, we just use the current time
        }
    end

    local function delete(id: string)
        lockdownkeys.remove(id)
    end

    self.list = list
    self.get = get
    self.set = set
    self.delete = delete

    return self
end

return LockdownManager