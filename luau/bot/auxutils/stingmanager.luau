--!strict
local datetime = require "@antiraid/datetime"
local discord = require "@discord-types/apiTypes"
local Primitives = require "@antiraid-core/primitives"
local KeyExpiryManager = require "@antiraid-ext/keyexpirymanager"

export type Sting = {
    stingid: string,
    userid: discord.Snowflake,
    modid: discord.Snowflake?,
    stings: number,
    reason: string,
    created_at: datetime.DateTime,
    expires_at: datetime.DateTime,
}

export type CreateSting = {
    userid: discord.Snowflake,
    modid: discord.Snowflake?,
    stings: number,
    reason: string,
    expiry: datetime.TimeDelta,
}

--[[
    Manages the moderation 'stings' (think bee stings!) of a user.

    Stings are internally stored in the following format:
]]
export type StingManager = {
    --- Gets all the stings a user has accrued
    getStingsOnUser: (userId: discord.Snowflake) -> {[string]: Sting},
    --- Gets a single sting by its ID
    getSting: (stingId: string) -> Sting?,
    --- Creates a new sting for a user returning a Sting
    createUserSting: (sting: CreateSting) -> Sting,
    --- Deletes a sting.
    deleteSting: (stingId: string, mod: string?, auditReason: string) -> Sting?,
    --- Compactly stringifies a sting
    stingCompactString: (sting: Sting) -> string,
}

export type StingExpiryData = {
    userid: discord.Snowflake,
    modid: discord.Snowflake?,
    stings: number,
    reason: string,
}

local function StingManager(ctx: Primitives.TemplateContext): StingManager
    local self = {}
    local cachedUserStings: {[string]: {[string]: Sting}} = {}

    local function _parseStingData(item: KeyExpiryManager.ExpiringKeyRecord<StingExpiryData>): Sting
        return {
            stingid = item.key,
            userid = item.value.userid,
            modid = item.value.modid,
            created_at = item.createdat,
            reason = item.value.reason,
            expires_at = item.expiresat,
            stings = item.value.stings
        }
    end

    local stingexpiry: KeyExpiryManager.KeyExpiryManager<StingExpiryData> 
    stingexpiry = KeyExpiryManager<<StingExpiryData>>(ctx, "builtins.stings", function(record) 
        -- Fetch the sting
        local userid = record.value.userid
        local stingid = record.key

        -- Dispatch event before expiration so that other templates can react to the sting expiration if needed
        ctx.loop.dispatch({
            name = "BuiltinsStingExpire",
            author = nil,
            data = _parseStingData(record),
        })

        if cachedUserStings[userid] and cachedUserStings[userid][stingid] then
            cachedUserStings[userid][stingid] = nil -- cache invalidation
            if next(cachedUserStings[userid]) == nil then cachedUserStings[userid] = nil end
        end
        
        return nil
    end)

    local function deleteSting(stingid: string, mod: string?, auditReason: string): Sting?
        if not stingid or not auditReason then
            error("User ID, Sting ID or Audit Reason is nil")
        end

        local record = stingexpiry.get(stingid)
        if not record then return nil end
        local csting = _parseStingData(record)

        -- Dispatch event before deletion so that other templates can react to the sting deletion if needed
        ctx.loop.dispatch({
            name = "BuiltinsStingDelete",
            author = mod,
            data = csting,
        })

        -- Delete the expiration task
        stingexpiry.remove(stingid)
        if cachedUserStings[csting.userid] and cachedUserStings[csting.userid][stingid] then
            cachedUserStings[csting.userid][stingid] = nil -- cache invalidation
            if next(cachedUserStings[csting.userid]) == nil then cachedUserStings[csting.userid] = nil end
        end

        return csting
    end

    local function getStingsOnUser(userid: discord.Snowflake): {[string]: Sting}
        if stingexpiry.count() == 0 then return {}
        elseif cachedUserStings[userid] then return cachedUserStings[userid] 
        else 
            local stings = stingexpiry.list()
            cachedUserStings[userid] = {}
            for _, sting in stings do 
                if sting.value.userid ~= userid then continue end
                cachedUserStings[userid][sting.key] = _parseStingData(sting)
            end
            return cachedUserStings[userid]
        end
    end

    local function getSting(stingid: string): Sting?
        local sting = stingexpiry.get(stingid)
        if sting then return _parseStingData(sting) end
        return nil
    end

    local function createUserSting(sting: CreateSting): Sting
        local csting: StingExpiryData = {
            userid = sting.userid,
            reason = sting.reason,
            modid = sting.modid,
            stings = sting.stings or 1,
        }
        local stingid = stingexpiry.add(sting.expiry, csting)

        local cesting = stingexpiry.get(stingid)
        assert(cesting, "internal error: template not inserted by add call")
        local parsedcsting = _parseStingData(cesting)

        -- Cache fix
        if cachedUserStings[sting.userid] then cachedUserStings[sting.userid][stingid] = parsedcsting end

        -- Dispatch event upon creation so that other templates can react to the sting creation if needed
        ctx.loop.dispatch({
            name = "BuiltinsStingCreated",
            author = sting.modid,
            data = parsedcsting,
        })

        return parsedcsting
    end

    local function stingCompactString(sting: Sting): string
        local expiresAt = if sting.expires_at then "<t:" .. sting.expires_at.timestamp_seconds .. ">" else "Never"
        local mod = if sting.modid then "<@" .. sting.modid .. ">" else "System"
        local reason = if #sting.reason > 0 then sting.reason else "No reason provided"
        if #reason > 128 then
            reason = reason:sub(1, 128) .. "..."
        end
        -- "Sting by " .. if maxSting.modId then "<@" .. maxSting.modId .. ">" else "System" .. " with ID of ``" .. maxSting.stingId .. "``",
        return string.format("Sting by %s with ID of ``%s``\nReason: %s\nCreated at: <t:%d>\nExpires at: %s\nSting count: %d", mod, sting.stingid, reason, sting.created_at.timestamp_seconds, expiresAt, sting.stings)
    end

    -- Save to self
    self.getStingsOnUser = getStingsOnUser
    self.createUserSting = createUserSting
    self.deleteSting = deleteSting
    self.getSting = getSting
    self.stingCompactString = stingCompactString

    return self
end

return StingManager