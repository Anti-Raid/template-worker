--!strict

local timeouts: { [number]: thread } = {}
local intervals: { [number]: thread } = {}

local nextTimeoutId = 1
local nextIntervalId = 1

--- Implementation of setTimeout for the AntiRaid runtime.
---
--- Functions like JavaScript's setTimeout.
---
--- @param callback The function to call after the delay.
--- @param delay The delay in milliseconds. If a string is passed, it will be converted to a number.
--- @param ... Additional arguments to pass to the callback function.
--- @return tid The timeout ID, which can be used to cancel the timeout with `clearTimeout`.
local function setTimeout(callback: (...any) -> (), delay: number?, ...): number 
    local args = table.pack(...)
    
    if type(delay) == "string" then delay = tonumber(delay) end
    delay = delay or 0

    local tid = nextTimeoutId
    nextTimeoutId += 1

    local th = task.delay(delay / 1000, function() 
        timeouts[tid] = nil 
        callback(table.unpack(args, 1, args.n))
    end)

    timeouts[tid] = th
    return tid
end

--- Implementation of clearTimeout for the AntiRaid runtime.
---
--- Functions like JavaScript's clearTimeout.
---
--- @param tid The timeout ID to cancel.
--- @return nil
local function clearTimeout(tid: number)
    local th = timeouts[tid]
    if th then
        timeouts[tid] = nil
        if coroutine.status(th) ~= "dead" then
            task.cancel(th)
        end
    end
end

--- Implementation of setInterval for the AntiRaid runtime.
---
--- Functions like JavaScript's setInterval.
---
--- @param callback The function to call at each interval.
--- @param delay The delay in milliseconds. If a string is passed, it will be converted to a number.
--- @param ... Additional arguments to pass to the callback function.
--- @return tid The interval ID, which can be used to cancel the interval with `clearInterval`.
local function setInterval(callback: (...any) -> (), delay: number?, ...): number    
    local args = table.pack(...)
    
    if type(delay) == "string" then delay = tonumber(delay) end
    local rdelay = delay or 500

    local tid = nextIntervalId
    nextIntervalId += 1

    local th = task.spawn(function()
        while true do
            task.wait(rdelay / 1000)
            callback(table.unpack(args, 1, args.n))
        end
    end)

    intervals[tid] = th
    return tid
end

--- Implementation of clearInterval for the AntiRaid runtime.
---
--- Functions like JavaScript's clearInterval.
---
--- @param tid The interval ID to cancel.
--- @return nil
local function clearInterval(tid: number)
    local th = intervals[tid]
    if th then
        -- 1. Free memory
        intervals[tid] = nil
        -- 2. Cancel the thread to stop the loop
        if coroutine.status(th) ~= "dead" then
            task.cancel(th)
        end
    end
end

return {
    setTimeout = setTimeout,
    clearTimeout = clearTimeout,
    setInterval = setInterval,
    clearInterval = clearInterval,
}