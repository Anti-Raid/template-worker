local datetime = require"@antiraid/datetime"

--- Format a TimeDelta into a human-readable string up to a week
local function formatTimeDelta(delta: datetime.TimeDelta): string
    local seconds = delta.seconds
    local minutes = math.floor(seconds / 60)
    seconds = seconds % 60

    if minutes == 0 then
        return ("%02ds"):format(seconds)
    end

    local hours = math.floor(minutes / 60)
    minutes = minutes % 60

    if hours == 0 then
        return ("%02dm %02ds"):format(minutes, seconds)
    end


    local days = math.floor(hours / 24)
    hours = hours % 24

    if days == 0 then
        return ("%02dh %02dm %02ds"):format(hours, minutes, seconds)
    end

    local weeks = math.floor(days / 7)
    days = days % 7

    if weeks == 0 then
        return ("%02dd %02dh %02dm %02ds"):format(days, hours, minutes, seconds)
    end

    return ("%02dw %02dd %02dh %02dm %02ds"):format(weeks, days, hours, minutes, seconds)
end

return formatTimeDelta