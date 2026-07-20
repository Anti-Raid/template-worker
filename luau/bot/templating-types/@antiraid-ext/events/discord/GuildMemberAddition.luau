local Primitives = require("@antiraid-core/primitives")
local discordgateway = require("@discord-types/gatewayTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- GuildMemberAddition
---
--- Dispatched when a user joins a guild.
---
--- Provides the guild’s id and the user’s member data.
local function GuildMemberAddition(callback: (ctx: Primitives.TemplateContext, member: discordgateway.GuildMemberAddPayload) -> ())
    return createTab("GUILD_MEMBER_ADD", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildMemberAddition