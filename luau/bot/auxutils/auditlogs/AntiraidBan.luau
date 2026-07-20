--!strict
local Primitives = require "@antiraid-core/primitives"
local Custom = require "@antiraid-ext/events/antiraid/Custom"
local helpers = require "./helpers"

--- Audit log handler for AntiraidBan events
return Custom("AntiraidBan")(function(ctx: Primitives.TemplateContext, data: helpers.AntiraidBanData)
    local userid = data.userid
    local reason = data.reason or "No reason provided"
    local tempdata = data.tempdata

    local description = string.format(
        "%s was banned from the server",
        helpers.userMention(userid)
    )

    local fields: {{name: string, value: string, inline: boolean?}} = {
        { name = "Reason", value = helpers.truncate(reason, 1024), inline = false },
    }

    if tempdata and tempdata.duration then
        table.insert(fields, { name = "Duration", value = tostring(tempdata.duration) .. "s", inline = true })
    end

    if data.delete_message_seconds and data.delete_message_seconds > 0 then
        table.insert(fields, { name = "Messages Deleted", value = tostring(data.delete_message_seconds) .. "s worth", inline = true })
    end

    helpers.sendAuditLog(ctx, {
        title = if tempdata then "Member Temporarily Banned" else "Member Banned",
        description = description,
        color = 0xFF0000,
        fields = fields,
    }, "antiraid_ban")
end)
