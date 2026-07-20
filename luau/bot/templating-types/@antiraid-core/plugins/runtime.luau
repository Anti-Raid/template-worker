--!strict

local datetime = require("@antiraid/datetime")
local typesext = require"@antiraid/typesext"
local blob = require"@antiraid-core/blob"
local khronosvalue = require "@antiraid-core/khronosvalue"
local discord = require("@discord-types/apiTypes")
local discordsys = require("@antiraid-core/plugins/discord")

--- A result from a state operation
export type StateExecResult = {
    op: "Kv",

    --- The key of the record.
    key: string,
    --- The value of the record. This can be any type, depending on what was stored.
    value: khronosvalue.KhronosValue,
    --- The scopes the key has
    scope: string,
    --- The timestamp when the record was created, in ISO 8601 format.
    created_at: datetime.DateTime,
    --- The timestamp when the record was last updated, in ISO 8601 format.
    last_updated_at: datetime.DateTime,
} | {
    op: "KvWithBlob",

    --- The key of the record.
    key: string,
    --- The value of the record. This can be any type, depending on what was stored.
    value: khronosvalue.KhronosValue,
    --- The blob associated with the record, if any. This is used for storing small binary data that can be downloadable
    blob: blob.Blob?,
    --- The scopes the key has
    scope: string,
    --- The timestamp when the record was created, in ISO 8601 format.
    created_at: datetime.DateTime,
    --- The timestamp when the record was last updated, in ISO 8601 format.
    last_updated_at: datetime.DateTime,
} | {
    op: "KvSignUrl",

    --- Url of the expiry
    url: string,
    --- The number of seconds until the URL expires. After this time, the URL will no longer work.
    expiry: number,
} | {
    op: "GlobalKv",

    read key: string,

    --- The version of the record
    read version: number,

    --- The owner id of the record (for global kvs, this is the tenant id)
    read owner_id: string,

    --- The owner type of the record (user or guild)
    read owner_type: string,

    --- Short description of the record
    read short: string,

    --- Public metadata about the key-value
    read public_metadata: any,

    --- Contents of vfs is readable or not (paid templates)
    read public_data: boolean,

    --- Review state of the record (pending, approved, rejected)
    read review_state: string,

    --- An optional long description of the record
    read long: string?,

    --- An optional price for the record (if the record is for sale)
    read price: number?,

    --- The timestamp when the record was created
    read created_at: datetime.DateTime,

    --- The timestamp when the record was last updated at
    read last_updated_at: datetime.DateTime,
} | {
    op: "GlobalKvData",
    data: any
} | {
    op: "GlobalKvDataOpaque",
    data: any, -- todo: add opaque type,
}

--- Internal tenant state of the running VM
export type TenantState = {
    --- The list of events the guild is (globally) subscribed to
    events: {[string]: {[string]: boolean}},
    --- Flags (such as whether we need to load in more data etc.)
    flags: number
}

export type Id = {
    read tenant_type: "guild" | "user",
    read tenant_id: string
}

export type LtcMessage = {
    op: "Msg",
    msg: khronosvalue.KhronosValue,
    id: number,
} | {
    op: "Close",
    id: number,
}

export type CtlMessage = {
    type: "NewConn",
    id: number,
    user_id: string,
} | {
    type: "CloseConn",
    id: number,
} | {
    type: "Msg",
    id: number,
    msg: khronosvalue.KhronosValue,
}

export type StreamTx = {
    --- @yields
    send: (self: StreamTx, msg: LtcMessage) -> (),

    --- @noyield
    pubsync: (self: StreamTx, conn_ids: {number}, msg: any) -> (),
    
    --- @yields
    pub: (self: StreamTx, conn_ids: {number}, msg: any) -> ()
}

export type StreamRx = {
    --- @yields
    recv: (self: StreamRx) -> CtlMessage
}

export type BaseTenantData = {
    read bot: discord.UserObject,
    read id: Id,
    read dispatchable_events: {string},
    read base_vfs: { [string]: typesext.Vfs },
    read support_server: string,
    read website: string,
    read stream_tx: StreamTx,
    read stream_rx: StreamRx,
}

export type StateOp = {
    op: "KvFind",
    query: string,
    scope: string
} | {
    op: "KvGet",
    key: string,
    scope: string
} | {
    op: "KvGetWithBlob",
    key: string,
    scope: string
} | {
    op: "KvSignUrl",
    key: string,
    scope: string,
} | {
    op: "KvSet",
    key: string,
    scope: string,
    value: khronosvalue.KhronosValue,
    blob: blob.BlobTaker?
} | {
    op: "KvDelete",
    key: string,
    scope: string
} | {
    op: "SubscribeEvent",
    event: string,
    system: string
} | {
    op: "UnsubscribeEvent",
    event: string,
    system: string
} | {
    op: "GlobalKvFind",
    query: string,
    scope: string
} | {
    op: "GlobalKvGet",
    key: string,
    version: number,
    scope: string
} | {
    op: "GlobalKvCreate",
    key: string,
    version: number,
    short: string,
    public_metadata: khronosvalue.KhronosValue,
    scope: string,
    public_data: boolean,
    long: string?,
    data: khronosvalue.KhronosValue
} | {
    op: "GlobalKvDelete",
    key: string,
    version: number,
    scope: string
}

export type CdnCall = { op: "DownloadFile", url: string, as_buffer: boolean } -- only discord cdn urls are supported
export type CdnResult = { op: "Blob", data: blob.Blob } | { op: "Buffer", data: buffer }

export type MetaCall = { op: "GetStats" } 
export type MetaResult = { op: "Stats", total_guilds: number, total_users: number, last_started_at: datetime.DateTime, } 

--- The arguments to be passed into a system call
export type SyscallArgs = {
    op: "State",
    --- A set of 'state operations' to perform. Depending on implementation, this may end up performing a call
    --- into the master via grpc or otherwise (hence making 'State' ops expensive) but this purely a implementation
    --- detail that can and likely will change in the future
    ---
    --- # Guarantees 
    --- Antiraid guarantees that all 'state operations'
    --- of 'ops' will be performed atomically. That is, if any op in `ops` fails, the entire observable 
    --- db state remains unchanged/all changes from `op` are roll-backed.
    ops: {StateOp}
} | {
    op: "Cdn",
    --- Cdn allows for downloading Discord CDN urls *only*
    req: CdnCall
} | {
    op: "Discord",
    req: discordsys.DiscordRequest
} | {
    op: "Meta",
    --- Metadata related requests
    req: MetaCall
}

export type SyscallRet = {
    op: "State",
    res: {StateExecResult},
    new_tenant_state: TenantState,
} | {
    op: "Cdn",
    res: CdnResult
} | {
    op: "Discord",
    res: discordsys.DiscordResponse
} | {
    op: "Meta",
    res: MetaResult
}

export type RawSyscall = {
    --- @yields
    ---
    --- Performs a system call
    async: (self: RawSyscall, args: SyscallArgs) -> SyscallRet,
}

return {}