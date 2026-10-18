--[[
Web dashboard handlers for the built-in features.

Each event takes `{ op = string, ... }` from the dashboard (sent through the
msyscall `DispatchEvent` call, which sets `event.author` to the signed-in user)
and always answers `{ ok = true, ... }` or `{ ok = false, error = string }` so the
dashboard can show the reason without parsing a Lua traceback.

Permission checks are the same kittycat permissions the slash commands use.

  WebLockdowns  { op = "create", kind = "full" | "channel" | "role", target = string?, reason = string }
                    -> { ok, lockdown = { id, type, reason } } or { ok = false, needs_autofix = true, error }
                { op = "autofix", kind, target? }   -> { ok }
                { op = "remove", id = string }      -> { ok }
                { op = "removeall" }                -> { ok, removed = number }
  WebBackups    { op = "create", messages = boolean?, channels = {string}?, assets = {string}?, no_assets = boolean?,
                  max_messages = number?, per_channel = number?, password = string? } -> { ok, id }
                { op = "delete", id = string }      -> { ok }
                { op = "inspect", id = string, password = string? } -> { ok, backup = {...} }
                { op = "restore", id = string, password = string?, protected_channels = {string}?, protected_roles = {string}? } -> { ok }
  WebHoneypots  { op = "list" }                     -> { ok, honeypots = {...} }
                { op = "set", channel_id, bots_allowed = boolean, punishment = "ban" | "kick" } -> { ok }
                { op = "remove", channel_id }       -> { ok }
  WebAuditLogs  { op = "list" }                     -> { ok, logs = {...}, actions = {...} }
                { op = "set", channel_id, whats = {string} } -> { ok }
                { op = "remove", channel_id }       -> { ok }
  WebModeration { op = "kick" | "ban" | "unban", user_id, reason, delete_message_seconds = number?, duration_seconds = number? } -> { ok }

Times are unix milliseconds.
]]
local Primitives = require "@antiraid-core/primitives"
local createTab = require("@antiraid-ext/events/dispatch").createTab
local UserInfoManager = require "@antiraid-ext/utils/userinfo"
local modhierarchy = require "@antiraid-ext/utils/modhierarchy"
local sb = require "@antiraid-ext/utils/statusbuffer"
local datetime = require "@antiraid/datetime"
local managers = require "../managers/managers"
local AuditLogManager = require "../auditlogmanager"
local backupTypes = require "../backups/backups"
local createBackup = require "../backups/create/start"
local inspectimpl = require "../backups/restore/inspect"
local mode = require "../lockdowns/mode"
local FullServerLockdown = require "../lockdowns/modes/fullserverlockdown"
local SingleChannelLockdown = require "../lockdowns/modes/singlechannellockdown"
local RoleLockdown = require "../lockdowns/modes/rolelockdown"

type Req = {[string]: any}
type Res = {[string]: any}
type Handler = (ctx: Primitives.TemplateContext, author: string, req: Req) -> Res

--- Cap on names returned when inspecting a backup, to keep the response small
local INSPECT_LIST_LIMIT = 200
--- Same message cap the /backups create command defaults to
local DEFAULT_MAX_MESSAGES = 100
local DEFAULT_PER_CHANNEL = 100

local uim: UserInfoManager.UserInfoManager? = nil
local function userinfo(ctx: Primitives.TemplateContext, author: string): UserInfoManager.UserInfo
    if not uim then uim = UserInfoManager(ctx) end
    assert(uim, "Internal error: UserInfoManager is nil")
    return uim.get(author)
end

--- Errors unless the author has `cap` (or any of `caps`)
local function require_perm(ctx: Primitives.TemplateContext, author: string, ...: string): UserInfoManager.UserInfo
    local info = userinfo(ctx, author)
    local caps = {...}
    local lasterr: any = nil
    for _, cap in caps do
        local ok, err = pcall(modhierarchy.canModeratorDo, info, cap)
        if ok then return info end
        lasterr = err
    end
    error(lasterr)
end

local function str(req: Req, key: string, optional: boolean?): string
    local v = req[key]
    if v == nil and optional then return "" end
    if type(v) ~= "string" or v == "" then error(`Missing or invalid "{key}"`, 0) end
    return v
end

local function snowflake(req: Req, key: string): string
    local v = str(req, key)
    if not string.match(v, "^%d+$") then error(`"{key}" must be a Discord ID`, 0) end
    return v
end

local function strlist(req: Req, key: string): {string}
    local v = req[key]
    if v == nil then return {} end
    if type(v) ~= "table" then error(`"{key}" must be a list`, 0) end
    local out = {}
    for _, item in v do
        if type(item) ~= "string" then error(`"{key}" must be a list of strings`, 0) end
        local trimmed = item:gsub("%s+", "")
        if trimmed ~= "" then table.insert(out, trimmed) end
    end
    return out
end

local function num(req: Req, key: string): number?
    local v = req[key]
    if v == nil then return nil end
    if type(v) ~= "number" then error(`"{key}" must be a number`, 0) end
    return v
end

local function ms(dt: datetime.DateTime?): number?
    return if dt then dt.timestamp_millis else nil
end

--- Turns any thrown value into a message the dashboard can show as-is
local function errmessage(err: any): string
    if type(err) == "table" and err.type == "MissingPermissions" then
        return `You need the {err.perm} permission on AntiRaid to do this. Ask the server owner to grant it.`
    end
    if type(err) == "string" then return err end
    return tostring(err)
end

local function lockdownmode(req: Req): mode.LockdownMode
    local kind = str(req, "kind")
    if kind == "full" then
        return FullServerLockdown()
    elseif kind == "channel" then
        return SingleChannelLockdown(snowflake(req, "target"))
    elseif kind == "role" then
        return RoleLockdown(snowflake(req, "target"))
    end
    error(`Unknown lockdown kind "{kind}"`, 0)
end

local LOCKDOWN_PERMS = { full = "lockdowns.full", channel = "lockdowns.scl", role = "lockdowns.role" }

local function lockdowns(ctx: Primitives.TemplateContext, author: string, req: Req): Res
    local lockdownset = managers.getmanagers(ctx).lockdownset
    local op = req.op
    if op == "create" then
        require_perm(ctx, author, LOCKDOWN_PERMS[req.kind] or "lockdowns.full")
        local reason = str(req, "reason")
        local m = lockdownmode(req)
        local problem = lockdownset.test(m)
        if problem then
            return { ok = false, needs_autofix = true, error = problem }
        end
        local created = lockdownset.apply(m, reason)
        return { ok = true, lockdown = { id = created.id, type = created.type.string_form(), reason = created.reason } }
    elseif op == "autofix" then
        require_perm(ctx, author, LOCKDOWN_PERMS[req.kind] or "lockdowns.full")
        lockdownset.autofix(lockdownmode(req))
        return { ok = true }
    elseif op == "remove" then
        require_perm(ctx, author, "lockdowns.remove")
        lockdownset.remove(str(req, "id"))
        return { ok = true }
    elseif op == "removeall" then
        require_perm(ctx, author, "lockdowns.remove")
        local count = #lockdownset.manager.list()
        lockdownset.removeall()
        return { ok = true, removed = count }
    end
    error(`Unknown op "{tostring(op)}"`, 0)
end

local function backups(ctx: Primitives.TemplateContext, author: string, req: Req): Res
    local m = managers.getmanagers(ctx)
    local mdm = m.backupmetadata
    local op = req.op
    if op == "create" then
        require_perm(ctx, author, "backups.create")
        local assets = strlist(req, "assets")
        for _, a in assets do
            if a ~= "icon" and a ~= "splash" and a ~= "banner" then error(`Unknown asset "{a}"`, 0) end
        end
        local opts: backupTypes.BackupCreateOpts = {
            channels = strlist(req, "channels"),
            perChannel = num(req, "per_channel") or DEFAULT_PER_CHANNEL,
            maxMessages = num(req, "max_messages") or DEFAULT_MAX_MESSAGES,
            backupMessages = req.messages == true,
            backupGuildAssets = (if req.no_assets == true then {} elseif req.assets == nil then {"icon", "splash", "banner"} else assets) :: any,
            specialAllocations = {},
        }
        local password = str(req, "password", true)
        local built = createBackup(ctx, opts, if password ~= "" then password else nil, sb.StatusBuffer())
        mdm.set(built.filename, { createdby = author }, built.backup)
        return { ok = true, id = built.filename }
    elseif op == "delete" then
        require_perm(ctx, author, "backups.delete")
        local id = str(req, "id")
        if not mdm.get(id) then error(`Backup "{id}" does not exist`, 0) end
        mdm.remove(id)
        return { ok = true }
    elseif op == "inspect" then
        require_perm(ctx, author, "backups.create", "backups.restore")
        local password = str(req, "password", true)
        local loaded = inspectimpl(
            ctx,
            { type = "kv", key = str(req, "id") },
            if password ~= "" then password else nil,
            sb.StatusBuffer()
        ).getLoadedBackup()
        local core = loaded.core
        local roles, channels = {}, {}
        for _, r in core.guild.roles or {} do
            if #roles >= INSPECT_LIST_LIMIT then break end
            table.insert(roles, { id = r.id, name = r.name, color = r.color })
        end
        for _, c in core.channels do
            if #channels >= INSPECT_LIST_LIMIT then break end
            table.insert(channels, { id = c.id, name = c.name, type = c.type, parent_id = c.parent_id })
        end
        local messages = 0
        for _, list in core.messages or {} do messages += #list end
        local assets = {}
        for name in loaded.assets do table.insert(assets, name) end
        return {
            ok = true,
            backup = {
                guild_name = core.guild.name,
                guild_id = core.guild.id,
                role_count = if core.guild.roles then #core.guild.roles else 0,
                channel_count = #core.channels,
                message_count = messages,
                roles = roles,
                channels = channels,
                assets = assets,
                options = {
                    messages = core.options.backupMessages,
                    max_messages = core.options.maxMessages,
                    per_channel = core.options.perChannel,
                },
            },
        }
    elseif op == "restore" then
        require_perm(ctx, author, "backups.restore")
        local password = str(req, "password", true)
        m.backupcheckpointmanager.new({
            opts = {
                protectedChannels = strlist(req, "protected_channels"),
                protectedRoles = strlist(req, "protected_roles"),
            },
            enckey = if password ~= "" then password else nil,
            gre = { restoredRoles = {}, restoredChannels = {} },
            src = { type = "kv", key = str(req, "id") },
        }, sb.StatusBuffer())
        return { ok = true }
    end
    error(`Unknown op "{tostring(op)}"`, 0)
end

local function honeypots(ctx: Primitives.TemplateContext, author: string, req: Req): Res
    local hm = managers.getmanagers(ctx).honeypotmanager
    local op = req.op
    if op == "list" then
        require_perm(ctx, author, "honeypots.list")
        local out = {}
        for _, h in hm.list() do
            table.insert(out, { channel_id = h.channelid, bots_allowed = h.botallowed, punishment = h.punishment, created_at = ms(h.created_at) })
        end
        return { ok = true, honeypots = out }
    elseif op == "set" then
        require_perm(ctx, author, "honeypots.setup")
        local punishment = str(req, "punishment")
        if punishment ~= "ban" and punishment ~= "kick" then error("Punishment must be ban or kick", 0) end
        local channelid = snowflake(req, "channel_id")
        -- Updating an existing honeypot should not count against the limit
        if hm.has(channelid) then hm.delete(channelid) end
        hm.set({ channelid = channelid, botallowed = req.bots_allowed == true, punishment = punishment :: any })
        return { ok = true }
    elseif op == "remove" then
        require_perm(ctx, author, "honeypots.remove")
        local channelid = snowflake(req, "channel_id")
        if not hm.has(channelid) then error("That channel is not a honeypot", 0) end
        hm.delete(channelid)
        return { ok = true }
    end
    error(`Unknown op "{tostring(op)}"`, 0)
end

local function auditlogs(ctx: Primitives.TemplateContext, author: string, req: Req): Res
    local alm = managers.getmanagers(ctx).auditlogmanager
    local op = req.op
    if op == "list" then
        require_perm(ctx, author, "auditlog.set")
        local out = {}
        for _, l in alm.list() do
            table.insert(out, { channel_id = l.channel_id, whats = l.whats, created_at = ms(l.created_at) })
        end
        return { ok = true, logs = out, actions = AuditLogManager.actions }
    elseif op == "set" then
        require_perm(ctx, author, "auditlog.set")
        local whats = strlist(req, "whats")
        for _, w in whats do
            local known = false
            for _, a in AuditLogManager.actions do
                if a.value == w then known = true break end
            end
            if not known then error(`Unknown action "{w}"`, 0) end
        end
        alm.set({ channel_id = snowflake(req, "channel_id"), whats = whats :: any })
        return { ok = true }
    elseif op == "remove" then
        require_perm(ctx, author, "auditlog.set")
        alm.delete(snowflake(req, "channel_id"))
        return { ok = true }
    end
    error(`Unknown op "{tostring(op)}"`, 0)
end

local function moderation(ctx: Primitives.TemplateContext, author: string, req: Req): Res
    local op = req.op
    local userid = snowflake(req, "user_id")
    local reason = str(req, "reason")
    if op == "kick" then
        local info = userinfo(ctx, author)
        modhierarchy.canModeratorAffectTarget(ctx, info, userid, "moderation.kick")
        ctx.discord:remove_guild_member({ reason = reason, user_id = userid })
        ctx.loop.dispatch({ name = "AntiraidKick", author = info.userid, data = { userid = userid, reason = reason } })
        return { ok = true }
    elseif op == "ban" then
        local info = userinfo(ctx, author)
        modhierarchy.canModeratorAffectTarget(ctx, info, userid, "moderation.ban")
        local tempbankex = managers.getmanagers(ctx).tempbankex
        if tempbankex.get(userid) then error("This user already has an active tempban", 0) end
        local delete_message_seconds = num(req, "delete_message_seconds")
        local duration = num(req, "duration_seconds")
        local tempdata = nil
        if duration and duration > 0 then
            local key = tempbankex.addwithsecs(duration, { userid = userid, modid = info.userid, reason = reason }, userid)
            tempdata = { key = key, duration = duration }
        end
        ctx.discord:create_guild_ban({ delete_message_seconds = delete_message_seconds, reason = reason, user_id = userid })
        ctx.loop.dispatch({
            name = "AntiraidBan",
            author = info.userid,
            data = { userid = userid, reason = reason, delete_message_seconds = delete_message_seconds, tempdata = tempdata },
        })
        return { ok = true }
    elseif op == "unban" then
        local info = require_perm(ctx, author, "moderation.unban")
        local ban = ctx.discord:get_guild_ban(userid)
        if not ban.data then error("That user is not banned", 0) end
        ctx.discord:remove_guild_ban({ reason = reason, user_id = userid })
        local tempbankex = managers.getmanagers(ctx).tempbankex
        local tempdata = tempbankex.get(userid)
        if tempdata then tempbankex.remove(tempdata.key) end
        ctx.loop.dispatch({ name = "AntiraidUnban", author = info.userid, data = { userid = userid, reason = reason, tempbanrecord = tempdata } })
        return { ok = true }
    end
    error(`Unknown op "{tostring(op)}"`, 0)
end

local function webevent(name: string, handler: Handler)
    return createTab(name, function(ctx, event)
        local author = event.author
        if type(author) ~= "string" then return { ok = false, error = "This action needs a signed-in user" } end
        local req = event.data
        if type(req) ~= "table" then return { ok = false, error = "Missing request body" } end
        local ok, res = pcall(handler, ctx, author, req :: Req)
        if ok then return res end
        return { ok = false, error = errmessage(res) }
    end)
end

return {
    webevent("WebLockdowns", lockdowns),
    webevent("WebBackups", backups),
    webevent("WebHoneypots", honeypots),
    webevent("WebAuditLogs", auditlogs),
    webevent("WebModeration", moderation),
}
