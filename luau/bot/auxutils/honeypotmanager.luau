local Primitives = require "@antiraid-core/primitives"
local datetime = require "@antiraid/datetime"
local KeyManager = require "@antiraid-ext/keymanager"

local MAX_HONEYPOTS = 3 -- Maximum honeypots allowed per server

export type CreateHoneypot = {
    botallowed: boolean, -- Whether bots are allowed to message in the honeypot
    punishment: "ban" | "kick", -- Punishment to apply
    channelid: string, -- ID of the honeypot channel    
}

export type Honeypot = {
    channelid: string, -- ID of the honeypot channel
    botallowed: boolean, -- Whether bots are allowed to message in the honeypot
    punishment: "ban" | "kick", -- Punishment to apply
    created_at: datetime.DateTime, -- When the AFK was set
}

export type HoneypotManager = {
    --- Returns all setup honeypots on the server
    list: () -> {Honeypot},
    --- Returns if a channelid is setup as a honeypot
    has: (channelid: string) -> boolean,
    --- Sets a honeypot for a channel
    set: (data: CreateHoneypot) -> (),
    --- Deletes a honeypot
    delete: (channelid: string) -> (),
}

export type HoneypotData = {
    botallowed: boolean, -- Whether bots are allowed to message in the honeypot
    punishment: "ban" | "kick", -- Punishment to apply
}

--- A data fetcher for honeypots.
local function HoneypotManager(ctx: Primitives.TemplateContext): HoneypotManager
    local km = KeyManager<<HoneypotData>>(ctx, "builtins.honeypots")
    local cachedlist: {Honeypot}? = nil

    local self = {}

    local function _parseHoneypot(item: KeyManager.KeyRecord<HoneypotData>): Honeypot
        local channelid = item.key
        local createdAt = item.createdat

        return {
            channelid = channelid,
            botallowed = item.value.botallowed,
            punishment = item.value.punishment,
            created_at = createdAt,
        }
    end

    local function list(): {Honeypot}
        if cachedlist then return cachedlist end
        local data = km.list()

        local pots = {}
        for _, item in data do
            local pot = _parseHoneypot(item)
            table.insert(pots, pot)
        end

        pots = table.freeze(pots) :: {Honeypot}
        cachedlist = pots
        return pots
    end

    local function has(channelid: string) 
        return km.exists(channelid)
    end

    local function set(honeypot: CreateHoneypot): ()
        if km.count() >= MAX_HONEYPOTS then
            error("Maximum of 5 honeypots can be set at once")
        end
        ctx.loop.subscribe("MESSAGE_CREATE", "honeypot")
        km.add({
            botallowed = honeypot.botallowed,
            punishment = honeypot.punishment,
        }, honeypot.channelid)

        cachedlist = nil -- Invalidate cache
    end

    local function delete(channelid: string)
        km.remove(channelid)
        -- Unsubscribe if no more honeypots
        if km.count() == 0 then
            ctx.loop.unsubscribe("MESSAGE_CREATE", "honeypot")
        end

        cachedlist = nil -- Invalidate cache
    end

    self.list = list
    self.has = has
    self.set = set
    self.delete = delete

    return self
end

return HoneypotManager