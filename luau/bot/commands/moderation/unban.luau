local data = require"@antiraid-ext/frameworkv2/context"
local kexs = require"../../auxutils/managers/managers"
local units = require"@antiraid-ext/frameworkv2/unit"

local function unban(data: data.CommandContext): nil
    local modinfo = data.interaction.assertpermission("moderation.unban")

    local userid = data.args.userid("userid")
    local reason = data.args.string("reason")

    if not userid then
        return data.interaction.replysimpleembed("Cannot unban unknown user!", "No user specified", units.RED_COLOR)
    end

    if not reason or reason == "" then
        return data.interaction.replysimpleembed("Cannot unban user without a reason!", "Please provide a reason for the unban", units.RED_COLOR)
    end

    -- Check if a ban exists
    local ban = data.ctx.discord:get_guild_ban(userid)
    if not ban.data then
        return data.interaction.replysimpleembed("Cannot unban a user who is not banned!", "User <@" .. userid .. "> is not banned from this server!", units.RED_COLOR)
    end

    -- Then unban them
    data.ctx.discord:remove_guild_ban({
        reason = reason,
        user_id = userid,
    })

    -- Drop any tempmod key expiries for this user
    local tempbankex = kexs.getmanagers(data.ctx).tempbankex
    local tempdata = tempbankex.get(userid)
    if tempdata then
        tempbankex.remove(tempdata.key)
    end

    -- Then, dispatch a AntiraidUnban event
    data.ctx.loop.dispatch({
        name = "AntiraidUnban",
        author = modinfo.userid,
        data = {
            userid = userid,
            reason = reason,
            tempbanrecord = tempdata,
        },
    })

    return data.interaction.replysimpleembed("Successfully unbanned user", "User <@" .. userid .. "> was successfully unbanned from this server!", units.GREEN_COLOR)
end

return unban