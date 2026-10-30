local kc = require "@antiraid-core/kittycat"
local Primitives = require "@antiraid-core/primitives"
local userinfo = require "./userinfo"
local discordpermcalc = require "./discordpermcalc"
local permissions = require "@discord-types/permission"

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

--- Errors unless the user is the guild owner or holds the Discord Administrator permission.
---
--- Required for actions that apply arbitrary Discord permissions using the bot's authority (e.g. backup
--- restore can create roles and permission overwrites with any permission the bot itself holds), so that
--- an AntiRaid capability alone cannot be used to gain Discord permissions the user does not already have
local function assertDiscordAdministrator(userinfo: userinfo.UserInfo)
    if userinfo.guild_owner_id == userinfo.userid then
        return
    end

    if not discordpermcalc.HasPermission(integer.fromstring(userinfo.discord_permissions), permissions.Permissions.Administrator) then
        error("This action requires the Discord Administrator permission (in addition to the AntiRaid permission) as it can apply any Discord permission to roles and channels")
    end
end

return {
    assertDiscordAdministrator = assertDiscordAdministrator,
    canModeratorAffectTarget = canModeratorAffectTarget,
    canModeratorDo = canModeratorDo,
    canBotDoInChannel = canBotDoInChannel,
}
