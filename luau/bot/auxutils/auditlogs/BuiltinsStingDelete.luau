--!strict
local Primitives = require "@antiraid-core/primitives"
local Custom = require "@antiraid-ext/events/antiraid/Custom"
local helpers = require "./helpers"

--- Audit log handler for BuiltinsStingDelete events
return Custom("BuiltinsStingDelete")(function(ctx: Primitives.TemplateContext, data: helpers.StingEventData)
    local fields: {{name: string, value: string, inline: boolean?}} = {
        { name = "User", value = helpers.userMention(data.userId), inline = true },
        { name = "Moderator", value = helpers.modString(data.modId), inline = true },
        { name = "Sting Count", value = tostring(data.stings or 1), inline = true },
        { name = "Reason", value = helpers.truncate(data.reason or "No reason provided", 1024), inline = false },
        { name = "Sting ID", value = "``" .. (data.stingId or "unknown") .. "``", inline = true },
    }

    if data.created_at then
        table.insert(fields, { name = "Created At", value = "<t:" .. tostring(data.created_at.timestamp_seconds) .. ">", inline = true })
    end

    helpers.sendAuditLog(ctx, {
        title = "Sting Deleted",
        description = string.format(
            "A sting was deleted for %s",
            helpers.userMention(data.userId)
        ),
        color = 0x8B0000,
        fields = fields,
    }, "sting_delete")
end)
