local set = require"@antiraid-ext/set"
local priorityset = require("./priorityset")
local discord = require("@discord-types/apiTypes")

--- To ensure two lockdowns don't conflict with each other, we need some information about what all lockdowns are handling
--- along with what specificity they have
export type LockdownModeHandle = {
    roles: set.Set<discord.Snowflake>,
    channels: set.Set<discord.Snowflake>
}

--- To ensure two lockdowns don't conflict with each other, we need some information about what all lockdowns are handling
--- along with what specificity they have
local function LockdownModeHandle(): LockdownModeHandle
    local self = {}
    self.roles = set.Set() :: set.Set<discord.Snowflake>
    self.channels = set.Set() :: set.Set<discord.Snowflake>
    return self
end

--- To ensure two lockdowns don't conflict with each other, we need some information about what all lockdowns are handling
--- along with what specificity they have
export type LockdownModeHandles = {
    roles: priorityset.PrioritySet<discord.Snowflake>,
    channels: priorityset.PrioritySet<discord.Snowflake>,

    --- Adds a handle to the set given the specificity of the handle
    addhandle: (handle: LockdownModeHandle, specificity: number) -> (),
    --- Removes a handle from the set given the specificity of the handle
    removehandle: (handle: LockdownModeHandle, specificity: number) -> (),
    --- A role is locked if it contains all roles of the current *with a lower specificity*
    isrolelocked: (role: discord.Snowflake, specificity: number) -> boolean,
    --- A channel is locked if it contains all channel of the current *with a lower specificity*
    ischannellocked: (channel: discord.Snowflake, specificity: number) -> boolean,
    --- A handle is redundant if it contains all roles and channels of the current *with a lower specificity*
    isredundant: (other: LockdownModeHandle, other_spec: number) -> boolean
}

--- To ensure two lockdowns don't conflict with each other, we need some information about what all lockdowns are handling
--- along with what specificity they have
local function LockdownModeHandles(): LockdownModeHandles
    local self = {}
    self.roles = priorityset() :: priorityset.PrioritySet<discord.Snowflake>
    self.channels = priorityset() :: priorityset.PrioritySet<discord.Snowflake>

    local function addhandle(handle: LockdownModeHandle, specificity: number) 
        for role in handle.roles do 
            self.roles.add(role, specificity)
        end
        for channel in handle.channels do 
            self.channels.add(channel, specificity)
        end
    end

    local function removehandle(handle: LockdownModeHandle, specificity: number) 
        for role in handle.roles do 
            self.roles.remove(role, specificity)
        end
        for channel in handle.channels do 
            self.channels.remove(channel, specificity)
        end
    end

    local function isrolelocked(role: discord.Snowflake, specificity: number) 
        local currentspec = self.roles.highest_priority(role)
        if currentspec and currentspec >= specificity then return true end
        return false 
    end

    local function ischannellocked(channel: discord.Snowflake, specificity: number) 
        local currentspec = self.channels.highest_priority(channel)
        if currentspec and currentspec >= specificity then return true end
        return false 
    end

    local function isredundant(other: LockdownModeHandle, other_spec: number) 
        for role in other.roles do 
            local currentSpec = self.roles.highest_priority(role)
            if currentSpec and currentSpec >= other_spec then return false end
            if not currentSpec then return false end
        end

        for channel in other.channels do 
            local currentSpec = self.channels.highest_priority(channel)
            if currentSpec and currentSpec >= other_spec then return false end
            if not currentSpec then return false end
        end

        return true
    end

    self.addhandle = addhandle
    self.removehandle = removehandle
    self.isrolelocked = isrolelocked
    self.ischannellocked = ischannellocked
    self.isredundant = isredundant
    return self
end

--- To allow lockdowns to have access to the low-level data of other lockdowns,
--- this struct contains the roles and channels each lockdown knows about
export type LockdownSharableData = {
    role_permissions: {[discord.Snowflake]: string},
    channel_permissions: {[discord.Snowflake]: {discord.OverwriteObject}},
}

return {
    LockdownModeHandle = LockdownModeHandle,
    LockdownModeHandles = LockdownModeHandles,
}