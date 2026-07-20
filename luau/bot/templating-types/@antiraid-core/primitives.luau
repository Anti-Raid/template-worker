local runtimeP = require("@antiraid-core/plugins/runtime")
local Discord = require"@antiraid-ext/system/discord"

--- An event that has been dispatched to the template. 
--- 
--- In templates, this is ``arg``.
export type Event = {
    --- The name of the event.
    name: string,
    --- The authorized author of the event.
    ---
    --- Will be nil for events that are not explicitly user-authorized (e.g. using the website)
    author: string?,
    --- The data of the event.
    data: any,
}

export type DispatchResult = {
    type: "ok" | "err",
    id: string,
    value: any,
}

export type Dispatchable = {
    --- ID of the dispatchable
    read id: string,
    --- Fires an event 'in-thread' and returns the result directly.
    read runEvent: (rootctx: TemplateContext, event: Event) -> any,
}

export type SyscallFn = (args: runtimeP.SyscallArgs) -> runtimeP.SyscallRet

export type EventManager = {
    --- Subscribe to an event, adding the system to the event's refs
    ---
    --- If attachedop is set, the attached op will be run atomically with the event subscription
    read subscribe: (event: string, system: string, attachedop: runtimeP.StateOp?) -> (),

    --- Unsubscribe from the event loop for a system, removing the system from the events ref
    --- and automatically removing the event if no systems use it anymore
    read unsubscribe: (event: string, system: string) -> (),

    --- Returns if a system is subscribed to an event
    read isSubscribed: (event: string, system: string) -> boolean,

    --- Dispatch an event to all attached isolates, returning the result as desired
    read dispatch: (event: Event) -> {DispatchResult},

    --- Dispatch an event to the specified template isolates, returning the result as desired
    read dispatchSingle: (event: Event, id: string) -> any,

    --- Attach an isolate to the loop
    read attach: (isolate: Dispatchable) -> (),

    --- Remove/detach an isolate from the loop
    read detach: (id: string) -> (),
}

--- TemplateContext is a struct that represents the key context of a template
export type TemplateContext = {
    --- @yields
    ---
    --- Performs a system call
    ---
    --- This API is low-level and should be used with care.
    read syscall: SyscallFn,

    --- Returns the current tenant state 
    read tenantstate: () -> runtimeP.TenantState,

    --- Returns the current base tenant data
    read btd: () -> runtimeP.BaseTenantData,

    --- Returns the underlying discord client wrapper
    read discord: Discord.DiscordClient,

    --- Event manager/'loop' of the context
    read loop: EventManager,
    
    --- Stream manager for the context
    read stream: StreamManager,
}

export type StreamClientPayload = {
    t: "sub" | "unsub",
    n: string,
} | {
    t: "msg",
    n: string,
    msg: any,
}

export type StreamManager = {
    --- Publish a message to all connections subscribed to a specific topic
    read publish: (topic: string, msg: any) -> (),
}

return {}

