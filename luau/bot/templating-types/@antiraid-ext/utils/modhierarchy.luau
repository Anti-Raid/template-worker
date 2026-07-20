local kc = require "@antiraid-core/kittycat"
local Primitives = require "@antiraid-core/primitives"
local userinfo = require "./userinfo"

--- Errors if the moderator does not have cap `cap`. For most cases, `InteractionManager.assertpermission` is a better higher-level API to use here
local function canModeratorDo(userinfo: userinfo.UserInfo, cap: string) 
    if userinfo.guild_owner_id == userinfo.userid then 
        return
    end

    if not kc.has_perm(userinfo.kittycat_resolved_permissions, kc.Permission.from_string(cap)) then
        error({
            type = "MissingPermissions",
            perm = cap,
        })
    end
end

local function canBotDoInChannel(channelId: string, ctx: Primitives.TemplateContext, botNeededPerms: integer) 
    return ctx.discord:antiraid_check_channel_permissions({
        needed_permissions = tostring(botNeededPerms),
        channel_id = channelId,
        user_id = ctx.btd().bot.id,
    })
end

local function canModeratorAffectTarget(ctx: Primitives.TemplateContext, userinfo: userinfo.UserInfo, targetId: string, cap: string)
    if userinfo.userid == targetId then
        error("You cannot moderate yourself!")
    end
    
    if userinfo.guild_owner_id == targetId then
        error("Target user is the guild owner")
    elseif userinfo.guild_owner_id == userinfo.userid then 
        return
    end

    -- Check caps
    if not kc.has_perm(userinfo.kittycat_resolved_permissions, kc.Permission.from_string(cap)) then
        error({
            type = "MissingPermissions",
            perm = cap,
        })
    end

    -- Check hierarchy
    ctx.discord:antiraid_check_permissions_and_hierarchy({
        needed_permissions = "0",
        target_id = targetId,
        user_id = userinfo.userid,
    })
end

return {
    canModeratorAffectTarget = canModeratorAffectTarget,
    canModeratorDo = canModeratorDo,
    canBotDoInChannel = canBotDoInChannel,
}
