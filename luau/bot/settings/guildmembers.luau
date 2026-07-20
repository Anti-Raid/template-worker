--!strict
local array_metatable = require"@antiraid/interop".array_metatable
local verifyMemberPermOverrideChange = require"./permvalidator".verifyMemberPermOverrideChange
local data = require"@antiraid-ext/frameworkv2/context"
local settings = require"@antiraid-ext/frameworkv2/settings"
local kc = require"@antiraid-core/kittycat"

local createform = settings.FormBuilder()
:text("user_id", "User", nil, {type="Member"})
:array_text("perm_overrides", "Permission Overrides")
:button("create", "Give Member Permissions", "Primary", true)

local updateform = settings.FormBuilder()
:text("user_id", "User", { disabled = true }, {type="Member"})
:array_text("perm_overrides", "Permission Overrides")
:text("created_at", "Created At", { disabled = true })
:button("delete", "Remove Member Permissions", "Danger", false) -- delete the setting
:button("update", "Update Permissions", "Primary", true)

local function view(data: data.Framework) 
    verifyMemberPermOverrideChange({op = "View"})

    local result = setmetatable({}, array_metatable)

    local mpoe = data.userinfomanager.getMemberPermissionOverrides()

    for _, memberPermissions in mpoe do             
        local permsMap = {} 
        for _, _perm in memberPermissions.perm_overrides do 
            table.insert(permsMap, tostring(_perm))
        end
        
        table.insert(result, {
            user_id = memberPermissions.userid,
            perm_overrides = permsMap,
            created_at = memberPermissions.createdat,
        })
    end

    return result
end

local function create(ctx: data.SettingsFormActionContext) 
    local data = ctx.framework
    local author = ctx.author
    local userid = ctx.argstring("user_id")
    local permoverrides = ctx.argstringlist("perm_overrides")
    
    local targetUserInfo = data.userinfomanager.get(userid)
    local authorUserinfo = data.userinfomanager.get(author)

    verifyMemberPermOverrideChange({op = "Set" :: "Set", authorUserInfo = authorUserinfo, targetUserInfo = targetUserInfo, perm_overrides = permoverrides, author = author})

    local permsMapped = {}
    for _, perm in permoverrides do 
        table.insert(permsMapped, kc.Permission.from_string(perm))
    end    

    data.userinfomanager.setMemberPermissionOverrides(userid, permsMapped)
end

local function update(ctx: data.SettingsFormActionContext) 
    local data = ctx.framework
    local author = ctx.author
    local userid = ctx.form_id -- the form id the buttons clicked in is the user id of the permission (override) to delete

    local targetUserInfo = data.userinfomanager.get(userid)

    if not targetUserInfo.permission_override_entry.createdat then
        error("This user does not have any permission overrides set. Please create them first.")
    end

    local authorUserinfo = data.userinfomanager.get(author)

    if ctx.action_button_id == "update" then 
        local permoverrides = ctx.argstringlist("perm_overrides")
        
        verifyMemberPermOverrideChange({op = "Set" :: "Set", authorUserInfo = authorUserinfo, targetUserInfo = targetUserInfo, perm_overrides = permoverrides, author = author})

        local permsMapped = {}
        for _, perm in permoverrides do 
            table.insert(permsMapped, kc.Permission.from_string(perm))
        end    

        data.userinfomanager.setMemberPermissionOverrides(userid, permsMapped)
    elseif ctx.action_button_id == "delete" then
        verifyMemberPermOverrideChange({op = "Delete" :: "Delete", authorUserInfo = authorUserinfo, targetUserInfo = targetUserInfo, author = author})

        data.userinfomanager.deleteMemberPermissionOverrides(userid)
    end
end

local function fetch(p: settings.PageBuilder<data.Framework>)
    p
    :section("mp", "Member Permissions", "Manage what your members can do on AntiRaid", function(sb: settings.SectionBuilder<data.Framework>)
        sb
        :formset(
            "mp_create", 
            createform,
            false
        )
        :display({type = "Header" :: "Header", text = "Current Member Permissions"})
        :formset(
            "mp_update", 
            updateform,
            true
        )
    end)

    -- add it to the form
    p:addformdata("mp_create", { id = "mp_create_form", title = "New Member Permissions", data = {
        user_id = "",
        perm_overrides = setmetatable({}, array_metatable)
    } })
    local mpd = view(p.data)
    for _, mp in mpd do 
        p:addformdata("mp_update", { id = mp.user_id, title = mp.user_id, data = mp :: any })
    end
end

return {
    fetch = fetch,
    create = create,
    update = update
}