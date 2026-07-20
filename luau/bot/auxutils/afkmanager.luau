--!strict
local Primitives = require "@antiraid-core/primitives"
local KeyExpiryManager = require "@antiraid-ext/keyexpirymanager"
local datetime = require "@antiraid/datetime"

export type CreateAFK = {
    user_id: string, -- ID of the user to set AFK for
    channel_id: string?, -- ID of the channel to send an optional AFK message to when the AFK expires
    reason: string, -- Reason for the AFK status
    duration: datetime.TimeDelta, -- Duration for which the user should be AFK
}

export type AFK = {
    user_id: string, -- ID of the user who is AFK
    reason: string, -- Reason for the AFK status
    expires_at: datetime.DateTime, -- When the AFK status expires
    created_at: datetime.DateTime, -- When the AFK was set
}

export type AFKManager = {
    --- Returns a list of all AFKs
    list: () -> {AFK},
    --- Returns the count of all AFKs we currently have
    count: () -> number,
    --- Returns a user's AFK if they have one
    get: (userid: string) -> AFK | nil,
    --- Sets a AFK for a user
    set: (data: CreateAFK) -> (),
    --- Deletes a AFK
    delete: (userid: string) -> (),
}

--- AFK expiry manager
export type AFKExpiryData = {
    reason: string,
}

--- A data fetcher for AFKs.
local function AFKManager(ctx: Primitives.TemplateContext): AFKManager
    local self = {}

    local afkexpiry = KeyExpiryManager<<AFKExpiryData>>(ctx, "builtins.afk", function(record) 
        local afk = self.get(record.key)
        if not afk then 
            return 
        end

        -- Dispatch an event with the result of the AFK expiry so other templates can react to it if needed
        ctx.loop.dispatch{
            name = "BuiltinsAfkExpiry", 
            data = { 
                userid = record.key,
                reason = record.value.reason,
                createdat = record.createdat, 
            },
            author = nil
        }
        return nil
    end)

    local function _parseAfk(item: KeyExpiryManager.ExpiringKeyRecord<AFKExpiryData>): AFK
        local reason = item.value.reason
        local createdAt = item.createdat
        local expiresAt = item.expiresat

        return {
            user_id = item.key,
            reason = reason,
            expires_at = expiresAt,
            created_at = createdAt,
        }
    end

    local function list(): {AFK}
        local data = afkexpiry.list()

        local afks = {}
        for _, item in data do
            local afk = _parseAfk(item)
            table.insert(afks, afk)
        end

        return afks
    end

    local function count(): number
        return afkexpiry.count()
    end

    local function get(userid: string): AFK | nil
        local item = afkexpiry.get(userid)
        return if item then _parseAfk(item) else nil
    end

    local function set(afk: CreateAFK): ()
        afkexpiry.add(afk.duration, {
            reason = afk.reason,
        }, afk.user_id)
        ctx.loop.subscribe("MESSAGE_CREATE", "afk")
        ctx.loop.dispatch{
            name = "BuiltinsAfkSet", 
            data = { 
                user_id = afk.user_id, 
                reason = afk.reason, 
                duration = afk.duration,
            },
            author = nil
        }
    end

    local function delete(userid: string)
        local afk = get(userid)
        if not afk then return end
        afkexpiry.remove(userid)

        -- Unsubscribe if no more afks
        if count() == 0 then
            ctx.loop.unsubscribe("MESSAGE_CREATE", "afk")
        end

        ctx.loop.dispatch{
            name = "BuiltinsAfkDelete", 
            data = { 
                user_id = userid,  
                afk = afk,
            }
        }
    end

    local function onAfkExpire(record: KeyExpiryManager.ExpiringKeyRecord<AFKExpiryData>)
        local afk = get(record.key)
        if not afk then 
            return 
        end
        -- Dispatch an event with the result of the AFK expiry so other templates can react to it if needed
        ctx.loop.dispatch{
            name = "BuiltinsAfkExpiry", 
            data = { 
                userid = record.key,
                reason = record.value.reason,
                createdat = record.createdat, 
            },
            author = nil
        }
    end

    self.list = list
    self.count = count
    self.get = get
    self.set = set
    self.delete = delete
    self.onAfkExpire = onAfkExpire

    return self
end

return AFKManager