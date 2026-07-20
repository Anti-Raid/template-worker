local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildRoleDelete
---
--- Dispatched when a role is deleted in a guild.
local function GuildRoleDelete(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.GuildRoleDeletePayload) -> ())
    return createTab("GUILD_ROLE_DELETE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildRoleDelete