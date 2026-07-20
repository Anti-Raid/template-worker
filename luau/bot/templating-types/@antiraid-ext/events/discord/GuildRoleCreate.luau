local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildRoleCreate
---
--- Dispatched when a role is created in a guild.
local function GuildRoleCreate(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.GuildRoleCreatePayload) -> ())
    return createTab("GUILD_ROLE_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildRoleCreate