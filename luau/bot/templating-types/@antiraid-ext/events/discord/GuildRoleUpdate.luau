local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildRoleUpdate
---
--- Dispatched when a role is updated in a guild.
local function GuildRoleUpdate(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.GuildRoleUpdatePayload) -> ())
    return createTab("GUILD_ROLE_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildRoleUpdate