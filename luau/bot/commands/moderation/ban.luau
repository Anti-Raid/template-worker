local data = require"@antiraid-ext/frameworkv2/context"
local kexs = require"../../auxutils/managers/managers"
local units = require"@antiraid-ext/frameworkv2/unit"

local function ban(data: data.CommandContext): nil
    local modinfo = data.interaction.assertpermission("moderation.ban")

    local userid = data.args.userid("user")
    local reason = data.args.string("reason")
    local delete_message_seconds = data.args.number("delete_message_seconds")
    local duration = data.args.time("duration", "durationunit")

    if not userid then
        return data.interaction.replysimpleembed("Cannot ban unknown user!", "No user specified", units.RED_COLOR)
    end

    if not reason or reason == "" then
        return data.interaction.replysimpleembed("Cannot ban user without a reason!", "Please provide a reason for the ban", units.RED_COLOR)
    end

    local tempdata = nil
    local tempbankex = kexs.getmanagers(data.ctx).tempbankex

    -- Ensure user isn't alr tempbanned
    if tempbankex.get(userid) then 
        return data.interaction.replysimpleembed("User already has an active tempban!", "This user is already tempbanned. Please either wait for the existing tempban to expire or remove it before applying a new one.", units.RED_COLOR)
    end

    -- If we need this to be temporary, schedule an unban linked to a key expiry
    if duration and duration > 0 then
        local key = tempbankex.addwithsecs(duration, {
            userid = userid,
            modid = modinfo.userid,
            reason = reason
        }, userid) -- Use the userid as the key for easy lookup when unbanning
        tempdata = {
            key = key,
            duration = duration
        }
    end

    -- First ban them
    data.ctx.discord:create_guild_ban({
        delete_message_seconds = delete_message_seconds,
        reason = reason,
        user_id = userid,
    })

    -- Then, dispatch a AntiraidBan event
    data.ctx.loop.dispatch({
        name = "AntiraidBan",
        author = modinfo.userid,
        data = {
            userid = userid,
            reason = reason,
            delete_message_seconds = delete_message_seconds,
            tempdata = tempdata
        },
    })
    
    return data.interaction.replysimpleembed("Successfully banned user!", "User <@" .. userid .. "> was successfully banned from this server!", units.GREEN_COLOR)
end

return ban