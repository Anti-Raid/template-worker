local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- GuildMemberRemoval
---
--- Dispatched when a user’s membership ends by leaving, getting kicked, or being banned.
---
--- Provides the guild’s id and the user’s data.
local function GuildMemberRemoval(callback: (ctx: Primitives.TemplateContext, payload: discordgateway.GuildMemberRemovePayload) -> ())
    return createTab("GUILD_MEMBER_REMOVE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildMemberRemoval