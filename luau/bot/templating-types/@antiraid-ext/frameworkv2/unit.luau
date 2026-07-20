local SECONDS_PER_SECOND = 1
local SECONDS_PER_MINUTE = 60
local SECONDS_PER_HOUR = 3600
local SECONDS_PER_DAY = 86400

local RED_COLOR = 0x00ff00
local YELLOW_COLOR = 0xffff00
local GREEN_COLOR = 0x00ff00

local function toseconds(n: number, unit: string): number? 
    if unit == "seconds" then return n
    elseif unit == "minutes" then return n * SECONDS_PER_MINUTE 
    elseif unit == "hours" then return n * SECONDS_PER_HOUR
    elseif unit == "days" then return n * SECONDS_PER_DAY
    else return nil
    end
end

return table.freeze{
    SECONDS_PER_SECOND = SECONDS_PER_SECOND,
    SECONDS_PER_MINUTE = SECONDS_PER_MINUTE,
    SECONDS_PER_HOUR = SECONDS_PER_HOUR,
    SECONDS_PER_DAY = SECONDS_PER_DAY,
    toseconds = toseconds,

    RED_COLOR = RED_COLOR,
    YELLOW_COLOR = YELLOW_COLOR,
    GREEN_COLOR = GREEN_COLOR
}