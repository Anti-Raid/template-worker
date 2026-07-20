local data = require"@antiraid-ext/frameworkv2/context"
local units = require"@antiraid-ext/frameworkv2/unit"

local function kick(data: data.CommandContext): nil
    local modinfo = data.interaction.assertpermission("moderation.kick")

    local userid = data.args.userid("user")
    local reason = data.args.string("reason")

    if not userid then
        return data.interaction.replysimpleembed("Cannot kick unknown user!", "No user specified", units.RED_COLOR)
    end

    if not reason or reason == "" then
        return data.interaction.replysimpleembed("Cannot kick user without a reason!", "Please provide a reason for kicking this user", units.RED_COLOR)
    end

    -- First kick them
    data.ctx.discord:remove_guild_member({
        reason = reason,
        user_id = userid,
    })

    -- Then, dispatch a AntiraidKick event
    data.ctx.loop.dispatch({
        name = "AntiraidKick",
        author = modinfo.userid,
        data = {
            userid = userid,
            reason = reason,
        },
    })
    
    return data.interaction.replysimpleembed("Successfully kicked user!", "User <@" .. userid .. "> was successfully kicked from this server!", units.GREEN_COLOR)
end

return kick