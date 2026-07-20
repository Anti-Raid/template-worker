local discord = require("@discord-types/apiTypes")
local UserInfo = require"@antiraid-ext/utils/userinfo"
local kc = require "@antiraid-core/kittycat"

export type VerifyMemberOpts = {
    op: "View"
} | {
    op: "Set",
    author: discord.Snowflake,
    perm_overrides: {string},
    authorUserInfo: UserInfo.UserInfo,
    targetUserInfo: UserInfo.UserInfo,
} | {
    op: "Delete",
    author: discord.Snowflake,
    authorUserInfo: UserInfo.UserInfo,
    targetUserInfo: UserInfo.UserInfo,
}

local function verifyMemberPermOverrideChange(vp: VerifyMemberOpts) 
    if vp.op == "View" then 
        return -- For now, we do not limit view access to guild members API
    end

    if vp.authorUserInfo.guild_owner_id == vp.author then
        return -- No further checks needed, owner can do anything, early return
    end

    if vp.op == "Set" then 
        if not kc.has_perm(vp.authorUserInfo.kittycat_resolved_permissions, kc.Permission.from_string("guild_members.update_permission_overrides")) then
            error("You do not have permission to set guild member permission overrides. Please ask an administrator to give you the 'guild_members.set_permission_overrides' permission.")
        end

        local permsMapped = {}
        for _, perm in vp.perm_overrides do 
            table.insert(permsMapped, kc.Permission.from_string(perm))
        end

        local ok, err = kc.check_patch_changes(vp.authorUserInfo.kittycat_resolved_permissions, vp.targetUserInfo.kittycat_resolved_permissions, permsMapped)
        if not ok then 
            error(kc.cpce_to_string(err)) 
        end
    elseif vp.op == "Delete" then
        if not kc.has_perm(vp.authorUserInfo.kittycat_resolved_permissions, kc.Permission.from_string("guild_members.update_permission_overrides")) then
            error("You do not have permission to delete guild member permission overrides. Please ask an administrator to give you the 'guild_members.delete_permission_overrides' permission.")
        end

        local ok, err = kc.check_patch_changes(vp.authorUserInfo.kittycat_resolved_permissions, vp.targetUserInfo.kittycat_resolved_permissions, {})
        if not ok then 
            error(kc.cpce_to_string(err))  
        end
    end
end

return {
    verifyMemberPermOverrideChange = verifyMemberPermOverrideChange
}