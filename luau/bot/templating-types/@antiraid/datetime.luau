local typesext = require "@antiraid/typesext"

export type TimeDelta = typeof(setmetatable({} :: {
    --- @field nanos: The number of nanoseconds in the time delta.
    nanos: number,
    --- @field micros: The number of microseconds in the time delta.
    micros: number,
    --- @field millis: The number of milliseconds in the time delta.
    millis: number,
    --- @field seconds: The number of seconds in the time delta.
    seconds: number,
    --- @field minutes: The number of minutes in the time delta.
    minutes: number,
    --- @field hours: The number of hours in the time delta.
    hours: number,
    --- @field days: The number of days in the time delta.
    days: number,
    --- @field weeks: The number of weeks in the time delta.
    weeks: number,
    --- @field as_secs: The total fractional seconds in the time delta (not yet released)
    as_secs: number,
    --- @function () -> string
    --- Returns an 'offset' string representation of the time delta.
    --- E.g. "+05:30" for 5 hours and 30 minutes.
    offset_string: (self: TimeDelta) -> string,
}, {} :: {
    __add: (TimeDelta, TimeDelta) -> TimeDelta,
    __sub: (TimeDelta, TimeDelta) -> TimeDelta,
    __le: (TimeDelta, TimeDelta) -> boolean,
    __lt: (TimeDelta, TimeDelta) -> boolean,
    __tostring: (TimeDelta) -> string
}))

--- @class TimeZone
--- @within TimeZone
--- A timezone object.
export type TimeZone = typeof(setmetatable({} :: {
    --- Parses a datetime string and returns a DateTime object.
    fromString: (self: TimeZone, datetime: string) -> DateTime,
    --- @function (year: number, month: number, day: number, hour: number, minute: number, second: number, all: boolean?) -> DateTime
    --- Translates a timestamp in UTC time to a datetime in the said specific timezone. If `all` is set to true, then multiple times
    --- may be returned in the case of ambiguous times, otherwise the first time is returned.
    utcToTz: (self: TimeZone, year: number, month: number, day: number, hours: number, minutes: number, secs: number) -> DateTime,
    --- @function (year: number, month: number, day: number, hour: number, minute: number, second: number, all: boolean?) -> DateTime
    --- Translates a timestamp in the specified timezone to a datetime in UTC. If `all` is set to true, then multiple times
    --- may be returned in the case of ambiguous times, otherwise the first time is returned.
    tzToUtc: (self: TimeZone, year: number, month: number, day: number, hours: number, minutes: number, secs: number) -> DateTime,
    --- @function (hour: number, minute: number, second: number) -> DateTime
    --- Translates a time of the current day in UTC time to a datetime in the said specific timezone
    timeUtcToTz: (self: TimeZone, hours: number, minutes: number, secs: number) -> DateTime,
    --- @function (hour: number, minute: number, second: number) -> DateTime
    --- Translates a time of the current day in the said specific timezone to a datetime in UTC
    timeTzToUtc: (self: TimeZone, hours: number, minutes: number, secs: number) -> DateTime,
    --- @function () -> DateTime
    --- Translates the current timestamp to a datetime in the said specific timezone
    now: (self: TimeZone) -> DateTime,
    --- Converts a unix timestamp to a datetime in the said specific timezone
    fromTime: (self: TimeZone, timestamp: typesext.i64) -> DateTime,
    --- Converts a unix timestamp in milliseconds to a datetime in the said specific timezone
    fromTimeMillis: (self: TimeZone, timestamp: typesext.i64) -> DateTime,
    --- Converts a unix timestamp in microseconds to a datetime in the said specific timezone
    fromTimeMicros: (self: TimeZone, timestamp: typesext.i64) -> DateTime,
}, {} :: {
    __tostring: (TimeZone) -> string,
    __eq: (TimeZone, TimeZone) -> boolean
}))

--- @class DateTime
--- @within DateTime
--- A datetime object. 
---
--- Supports addition/subtraction/equality with TimeDelta objects as well as comparisons with other DateTime objects.
export type DateTime = typeof(setmetatable({} :: {
    --- @field year
    --- The year of the datetime.
    year: number,
    --- @field month
    --- The month of the datetime.
    month: number,
    --- @field day
    --- The day of the datetime.
    day: number,
    --- @field hour
    --- The hour of the datetime.
    hour: number,
    --- @field minute
    --- The minute of the datetime.
    minute: number,
    --- @field second
    --- The second of the datetime.
    second: number,
    --- @field timestamp_seconds
    --- The timestamp in seconds of the datetime from the Unix epoch.
    timestamp_seconds: number,
    --- @field timestamp_millis
    --- The timestamp in milliseconds of the datetime from the Unix epoch.
    timestamp_millis: number,
    --- @field timestamp_micros
    --- The timestamp in microseconds of the datetime from the Unix epoch.
    timestamp_micros: number,
    --- @field timestamp_nanos
    --- The timestamp in nanoseconds of the datetime from the Unix epoch.
    timestamp_nanos: number,
    --- @field timezone: TimeZone
    --- The timezone of the datetime.
    timezone: TimeZone,
    --- @field base_offset: TimeDelta
    --- The base (non-DST) offset of the datetime.
    base_offset: TimeDelta,
    --- @field dst_offset: TimeDelta
    --- The additional DST offset of the datetime.
    dst_offset: TimeDelta,
    --- @function (TimeZone) -> DateTime
    --- Converts the datetime to the specified timezone.
    with_timezone: (self: DateTime, TimeZone) -> DateTime,
    --- @function (string) -> string
    --- Formats the datetime using the specified format string.
    format: (self: DateTime, string) -> string,
    --- @function (DateTime) -> TimeDelta
    --- Calculates the duration between the current datetime and another datetime.
    duration_since: (self: DateTime, DateTime) -> TimeDelta,
}, {} :: {
    __add: (DateTime, TimeDelta) -> DateTime,
    __sub: (DateTime, TimeDelta) -> DateTime,
    __le: (DateTime, DateTime) -> boolean,
    __lt: (DateTime, DateTime) -> boolean,
    __eq: (DateTime, DateTime) -> boolean,
    __tostring: (DateTime) -> string
}))

--- @function (timezone: string) -> TimeZone
--- @param timezone: string (The timezone to get the offset for.)
--- @return TimeZone (The timezone object.)
--- Returns a new Timezone object if the timezone is recognized/supported.
local function new(timezone: string): TimeZone
    error("Implemented internally in AntiRaid runtime!")
end

local UTC: TimeZone = {} :: TimeZone;

--- @function (weeks: number) -> TimeDelta
--- @param weeks: number (The number of weeks to create the TimeDelta object with.)
--- @return TimeDelta
--- Creates a new TimeDelta object with the specified number of weeks.
local function timedelta_weeks(weeks: typesext.i64): TimeDelta
    error("Implemented internally in AntiRaid runtime!")
end

--- @function (days: number) -> TimeDelta
--- @param days: number (The number of days to create the TimeDelta object with.)
--- @return TimeDelta
--- Creates a new TimeDelta object with the specified number of days.
local function timedelta_days(days: typesext.i64): TimeDelta
    error("Implemented internally in AntiRaid runtime!")
end

--- @function (hours: number) -> TimeDelta
--- @param hours: number (The number of hours to create the TimeDelta object with.)
--- @return TimeDelta
--- Creates a new TimeDelta object with the specified number of hours.
local function timedelta_hours(hours: typesext.i64): TimeDelta
    error("Implemented internally in AntiRaid runtime!")
end

--- @function (minutes: number) -> TimeDelta
--- @param minutes: number (The number of minutes to create the TimeDelta object with.)
--- @return TimeDelta
--- Creates a new TimeDelta object with the specified number of minutes.
local function timedelta_minutes(minutes: typesext.i64): TimeDelta
    error("Implemented internally in AntiRaid runtime!")
end

--- @function (seconds: number) -> TimeDelta
--- @param seconds: number (The number of seconds to create the TimeDelta object with.)
--- @return TimeDelta
--- Creates a new TimeDelta object with the specified number of seconds.
local function timedelta_seconds(seconds: typesext.i64): TimeDelta
    error("Implemented internally in AntiRaid runtime!")
end

--- @function (millis: number) -> TimeDelta
--- @param millis: number (The number of milliseconds to create the TimeDelta object with.)
--- @return TimeDelta
--- Creates a new TimeDelta object with the specified number of milliseconds.
local function timedelta_millis(millis: typesext.i64): TimeDelta
    error("Implemented internally in AntiRaid runtime!")
end

--- @function (micros: number) -> TimeDelta
--- @param micros: number (The number of microseconds to create the TimeDelta object with.)
--- @return TimeDelta
--- Creates a new TimeDelta object with the specified number of microseconds.
local function timedelta_micros(micros: typesext.i64): TimeDelta
    error("Implemented internally in AntiRaid runtime!")
end

--- @function (nanos: number) -> TimeDelta
--- @param nanos: number (The number of nanoseconds to create the TimeDelta object with.)
--- @return TimeDelta
--- Creates a new TimeDelta object with the specified number of nanoseconds.
local function timedelta_nanos(nanos: typesext.i64): TimeDelta
    error("Implemented internally in AntiRaid runtime!")
end
 
return {
    new = new,
    UTC = UTC,
    timedelta_weeks = timedelta_weeks,
    timedelta_days = timedelta_days,
    timedelta_hours = timedelta_hours,
    timedelta_minutes = timedelta_minutes,
    timedelta_seconds = timedelta_seconds,
    timedelta_millis = timedelta_millis,
    timedelta_micros = timedelta_micros,
    timedelta_nanos = timedelta_nanos
}