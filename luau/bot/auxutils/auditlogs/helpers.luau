--!strict
local Primitives = require "@antiraid-core/primitives"
local AuditLogManager = require "../auditlogmanager"
local datetime = require "@antiraid/datetime"
local discord = require "@discord-types/apiTypes"

-- Event data types matching the dispatch sites

--- Data dispatched by commands/moderation/ban.luau
export type AntiraidBanData = {
    userid: string,
    reason: string,
    delete_message_seconds: number,
    tempdata: {
        key: string,
        duration: number,
    }?,
}

--- Data dispatched by commands/moderation/kick.luau
export type AntiraidKickData = {
    userid: string,
    reason: string,
}

--- Data dispatched by commands/moderation/unban.luau
--- tempbanrecord comes from KeyExpiryManager.get() and may be nil if no temp ban existed
export type AntiraidUnbanData = {
    userid: string,
    reason: string,
    modid: string,
    tempbanrecord: {
        key: string,
        value: any,
        createdat: datetime.DateTime,
        expiresat: datetime.DateTime,
    }?,
}

--- Data dispatched by auxutils/stingmanager.luau (used for both BuiltinsStingCreated and BuiltinsStingDelete)
export type StingEventData = {
    stingId: string,
    userId: string,
    modId: string?,
    stings: number,
    reason: string,
    created_at: datetime.DateTime,
    expires_at: datetime.DateTime?,
}

export type AuditLogEmbed = {
    title: string,
    description: string,
    color: number,
    fields: {{name: string, value: string, inline: boolean?}}?,
}

--- Sends an audit log embed to all configured audit log channels
local function sendAuditLog(ctx: Primitives.TemplateContext, embed: AuditLogEmbed, what: AuditLogManager.What): ()
    local auditlogmanager = AuditLogManager.AuditLogManager(ctx)
    local logs = auditlogmanager.list()

    if #logs == 0 then
        return -- No audit log channels configured
    end

    -- Add timestamp footer
    local now = datetime.UTC:now()
    local fullEmbed: discord.EmbedObject = {
        title = embed.title,
        description = embed.description,
        color = embed.color,
        fields = embed.fields,
        footer = {
            text = "Audit Log | " .. now:format("%Y-%m-%d %H:%M:%S UTC"),
        },
    }

    -- While this is technically O(channels*whats), this is bounded by channels<=7 and whats<=5
    -- so this is in practice O(1) with a max of 35 iterations.
    for _, log in logs do
        local shouldsend = false
        for _, w in log.whats do 
            if w == what or w == "*" then
                shouldsend = true
                break
            end
        end
        if not shouldsend then continue end
        pcall(function()
            ctx.discord:create_message({
                channel_id = log.channel_id,
                data = {
                    embeds = { fullEmbed },
                },
            })
        end)
    end
end

--- Formats a user mention from an ID
local function userMention(userId: string?): string
    if not userId then
        return "Unknown"
    end
    return "<@" .. userId .. ">"
end

--- Formats a moderator string (mention or "System" if nil)
local function modString(modId: string?): string
    if not modId then
        return "System"
    end
    return "<@" .. modId .. ">"
end

--- Truncates a string to a max length with ellipsis
local function truncate(str: string, maxLen: number): string
    if #str > maxLen then
        return str:sub(1, maxLen - 3) .. "..."
    end
    return str
end

return {
    sendAuditLog = sendAuditLog,
    userMention = userMention,
    modString = modString,
    truncate = truncate,
}