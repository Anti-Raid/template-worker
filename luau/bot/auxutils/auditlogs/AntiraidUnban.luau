--!strict
local Primitives = require "@antiraid-core/primitives"
local Custom = require "@antiraid-ext/events/antiraid/Custom"
local helpers = require "./helpers"

--- Audit log handler for AntiraidUnban events
return Custom("AntiraidUnban")(function(ctx: Primitives.TemplateContext, data: helpers.AntiraidUnbanData)
    local userid = data.userid
    local reason = data.reason or "No reason provided"

    local fields: {{name: string, value: string, inline: boolean?}} = {
        { name = "Reason", value = helpers.truncate(reason, 1024), inline = false },
    }

    if data.tempbanrecord then
        table.insert(fields, { name = "Was Temp Ban", value = "Yes", inline = true })
    end

    helpers.sendAuditLog(ctx, {
        title = "Member Unbanned",
        description = string.format(
            "%s was unbanned from the server",
            helpers.userMention(userid)
        ),
        color = 0x00FF00,
        fields = fields,
    }, "antiraid_unban")
end)
