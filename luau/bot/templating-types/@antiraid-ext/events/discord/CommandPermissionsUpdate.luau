local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab

--- CommandPermissionsUpdate
local function CommandPermissionsUpdate(callback: (ctx: Primitives.TemplateContext, permission: discord.GuildApplicationCommandPermissionsObject) -> ())
    return createTab("APPLICATION_COMMAND_PERMISSIONS_UPDATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return CommandPermissionsUpdate