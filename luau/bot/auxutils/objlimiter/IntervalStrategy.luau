local datetime = require "@antiraid/datetime"
local types = require "./types"

type Interval = {
    --- At what interval to start limiting at
    Start: number,
    --- At what interval to stop limiting at
    Stop: number,
}

--- Given a a interval mode, returns either true or false if the id is limited based on this
--- specific interval mode (true = limited, false = not limited)
---
--- Modes: Absolute: Absolute limiting from {StartUnixTime} to {StopUnixTime}
--- TimePerDay: Limit every day between Start and Stop times in a given Timezone (if start > stop, it is implied
--- the interval wraps over midnight e.g., 10 PM to 2 AM)
local function Strategy(mode: types.IntervalMode): boolean
    if mode.type == "Absolute" then
        return os.time() >= mode.start and os.time() <= mode.stop 
    elseif mode.type == "TimePerDay" then
        local tz = datetime.new(mode.timezone)
        local utcStartTime = tz:timeTzToUtc(mode.start.hours, mode.start.minutes, mode.start.seconds)
        local utcStopTime = tz:timeTzToUtc(mode.stop.hours, mode.stop.minutes, mode.stop.seconds)
        local now = os.time()

        -- If start <= stop, the interval is within a single day (e.g., 9 AM to 5 PM).
		-- If start > stop, the interval wraps over midnight (e.g., 10 PM to 2 AM).
		if utcStartTime <= utcStopTime then 
            return now >= utcStartTime.timestamp_seconds and now <= utcStopTime.timestamp_seconds
        else
            return now >= utcStartTime.timestamp_seconds or now <= utcStopTime.timestamp_seconds
        end
    else
        return false
    end
end    
   
return Strategy