local datetime = require"@antiraid/datetime"

export type BroadcastTx<T> = {
    --- Once called, sends a value to the channel. If no recievers, returns (0, message), otherwise (num_recv, nil)
    read send: (self: BroadcastTx<T>, message: T) -> (number, T?),
    --- Creates a new receiver that will listen for all new messages from the point newsub is called
    read newsub: (self: BroadcastTx<T>) -> BroadcastRx<T>,
    --- Returns once the underlying stream channel is closed
    ---
    --- @yields
    read waitforclosed: (self: BroadcastTx<T>) -> (),
}

export type BroadcastRx<T> = {
    --- Waits for a value to be sent to the channel and receives it returning (T, nil, 0)
    ---
    --- If no more senders, returns (nil, "closed", 0)
    --- If the reciever has lagged, returns (nil, "lagged", amt)
    read recv: (self: BroadcastRx<T>) -> (T?, string?, number),
    --- Same as recv but with a timeout.
    read recvtimeout: (self: BroadcastRx<T>, timeout: datetime.TimeDelta) -> (T?, string?, number),
    --- Returns if the underlying stream channel is closed
    read isclosed: (self: BroadcastRx<T>) -> boolean,
}

--- A delay channel allows sending values that will be received after a specified delay
export type DelayChannel<T> = typeof(setmetatable({} :: {
    --- Adds a value to the delay queue, to be received after the specified delay
    --- and returns a handle that can be used to cancel the delayed message
    read add: (self: DelayChannel<T>, message: T, delay: datetime.TimeDelta) -> KeyHandle<T>,
    --- Waits for the next value in the delay queue to be ready and receives it
    ---
    --- Next should always only ever be called *once*. The exact behavior of calling next multiple times is undefined
    --- (and may result in either a error or the thread being perpetually yielded forever.)
    read next: (self: DelayChannel<T>) -> T,
    --- Clears all pending delayed messages from the delay queue
    read clear: (self: DelayChannel<T>) -> (),
}, {} :: {
    __len: (self: DelayChannel<T>) -> number,
}))

--- A handle to a delayed message in a DelayChannel, which can be used to cancel the message before it's sent
---
--- Note that the underlying key may change if the items expiry is too large (leading to the item being reinserted
--- periodically). This is not exposed to the user as it is an implementation detail.
export type KeyHandle<T> = typeof(setmetatable({} :: {
    --- Cancels the delayed message associated with this handle. Returns true if removed, else false
    --- 
    --- The value will be sent if the boolean is true
    cancel: (self: KeyHandle<T>) -> (boolean, T?),
}, {} :: {
    --- Returns true if the two KeyHandles point to the same delayed message, false otherwise
    __eq: (self: KeyHandle<T>, other: KeyHandle<T>) -> boolean,
}))

--- cap must be <= 5
local function BroadcastChannel<T>(cap: number): (BroadcastTx<T>, BroadcastRx<T>)
    error("Implemented internally in AntiRaid runtime!")
end

local function DelayChannel<T>(): DelayChannel<T>
    error("Implemented internally in AntiRaid runtime!")
end

return {
    BroadcastChannel = BroadcastChannel,
    DelayChannel = DelayChannel,
}