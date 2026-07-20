--!strict
local Primitives = require "@antiraid-core/primitives"
local Custom = require "@antiraid-ext/events/antiraid/Custom"
local helpers = require "./helpers"

--- Audit log handler for AntiraidKick events
return Custom("AntiraidKick")(function(ctx: Primitives.TemplateContext, data: helpers.AntiraidKickData)
    local userid = data.userid
    local reason = data.reason or "No reason provided"

    helpers.sendAuditLog(ctx, {
        title = "Member Kicked",
        description = string.format(
            "%s was kicked from the server",
            helpers.userMention(userid)
        ),
        color = 0xFF8C00,
        fields = {
            { name = "Reason", value = helpers.truncate(reason, 1024), inline = false },
        },
    }, "antiraid_kick")
end)
