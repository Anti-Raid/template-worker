--!strict
export type Hms = {
    hours: number,
    minutes: number,
    seconds: number
}

export type IntervalMode = {
    type: "Absolute",
    --- At what unix epoch to start limiting 
    start: number,
    --- At what unix epoch to stop limiting
    stop: number,
} | {
    type: "TimePerDay",
    --- At what time of day to start limiting at
    start: Hms,
    --- At what time of day to stop limiting at
    stop: Hms,
    --- The timezone these times are in
    timezone: string,
}

export type RateMode = {
    --- What gcra limiterId to use for rate limiting
    ---
    --- If multiple objects share the same limiterId, they will share the same rate limit
    --- and the first object which creates the limiter will set the rate limit parameters for all
    --- other objects using the same limiterId.
    limiterId: string,
    --- The key to use for limiting. Defaults to limiterId if not specified.
    limiterKey: string?,
    --- At what minimum count should the rate limit be applied. May be ignored if the limiterId is 
    --- already created.
    count: number,
    --- What rate does this minimum count have to be at. May be ignored if the limiterId is 
    --- already created.
    time: number,
}

export type ObjLimitBlock = {
    type: "Interval",
    mode: IntervalMode,
} | {
    type: "Rate",
    mode: RateMode
}

export type ObjLimit = {
    type: "Single",
    block: ObjLimitBlock,
} | {
    type: "Multiple",
    left: ObjLimit,
    right: ObjLimit,
    op: "AND" | "OR" | "XOR",
}

return {}