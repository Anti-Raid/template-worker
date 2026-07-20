--- Provides a simple ratelimiter using the GCRA algorithm
export type RateLimiter<BucketKey> = {
    --- Maximum number of actions allowed
    getCount: () -> number,
    --- Time period for the rate limit in seconds
    getPeriod: () -> number,
    --- Time interval between allowed actions in seconds
    --- This is calculated as period / count
    getEmissionInterval: () -> number, 
    --- Buckets for different keys, where each key has a time after which the next action is allowed
    getBuckets: () -> {[BucketKey]: number },  
    --- Attempt to perform an action
    --- @param key The key for which the action is being attempted
    --- @return boolean ``true`` if the action is allowed, ``false`` if it is denied (rate limited)
    attemptAction: (key: BucketKey) -> boolean
}

--- Return a new rate limiter instance
--- @param count Maximum number of actions allowed in the period
--- @param period Time period for the rate limit in seconds
--- @return RateLimiter<BucketKey> A new rate limiter instance
local function RateLimiter<BucketKey>(
    count: number,  -- Maximum number of actions allowed
    period: number  -- Time period for the rate limit
): RateLimiter<BucketKey>
    local emission_interval = period / count
    local buckets = {}

    local function getCount(): number
        return count
    end

    local function getPeriod(): number
        return period
    end

    local function getEmissionInterval()
        return emission_interval
    end

    local function getBuckets(): {[BucketKey]: number }
        return buckets
    end

    local function attemptAction(key: BucketKey): boolean
        local now = os.clock()
        
        local tat = buckets[key]

        if not tat then
            tat = now
            buckets[key] = tat
        end

        tat = math.max(tat, now) 
        local separation = tat - now
        local maxinterval = period - emission_interval

        if separation > maxinterval then
            return false
        else 
            local new_tat = math.max(tat, now) + emission_interval
            buckets[key] = new_tat
            return true
        end
    end

    local self = {}
    self.getCount = getCount
    self.getPeriod = getPeriod
    self.getEmissionInterval = getEmissionInterval
    self.getBuckets = getBuckets
    self.attemptAction = attemptAction

    return self
end

-- Test
if(_G.cli ~= nil) then
    local limiter = RateLimiter(5, 10)  -- 5 actions per 10 seconds

    -- Try to perform actions in a loop
    for i = 1, 100 do
        if limiter.attemptAction("abc") then
            print("Action " .. tostring(i) .. " allowed!")
        else
            print("Action " .. tostring(i) .. " denied (rate limited)!")
            task.wait(1)
        end
    end
end

return RateLimiter