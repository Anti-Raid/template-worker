local shellctx = require"./shellcontext"
local id = require"../common/id"

local function AskForLuaId(ctx: shellctx.ShellContext): id.LuaId
    while true do 
        local tenanttype = ctx:GetInput("Tenant Type (guild/user): ")
        if tenanttype == nil then error("Unexpected EOF") end
        tenanttype = tenanttype:lower()

        if tenanttype ~= "guild" and tenanttype ~= "user" then 
            ctx:Log("Invalid tenant type, please enter 'guild' or 'user'")
            continue
        end

        local tenantid = ctx:GetInput("Tenant ID: ")
        if tenantid == nil then error("Unexpected EOF") end

        return {
            tenant_type = if tenanttype == "guild" then "Guild" else "User",
            tenant_id = tenantid,
        }
    end
end

local function ConfirmChoice(ctx: shellctx.ShellContext, prompt: string): boolean
    while true do 
        local confirm = ctx:GetInput(`{prompt} (y/n): `)
        if confirm == nil then error("Unexpected EOF") end
        confirm = confirm:lower()
        if confirm == "y" then 
            return true
        elseif confirm == "n" then 
            return false
        else
            ctx:Log("Please enter 'y' or 'n'")
        end
    end
end

local function AskForStringList(ctx: shellctx.ShellContext, prompt: string): {string} 
    while true do
        local list = {}
        ctx:Log(`{prompt} (enter empty line to finish):`)
        while true do 
            local input = ctx:GetInput("> ")
            if input == nil then error("Unexpected EOF") end
            if input == "" then break end
            table.insert(list, input)
        end

        ctx:Log("You entered:", list)
        if not ConfirmChoice(ctx, "Is this correct?") then
            task.wait(1)
            continue
        end

        return list
    end
end

local function AskForNumber(ctx: shellctx.ShellContext, prompt: string): number
    while true do 
        local input = ctx:GetInput(prompt)
        if input == nil then error("Unexpected EOF") end

        local num = tonumber(input)
        if num == nil then 
            ctx:Log("Invalid number, please try again.")
            continue
        end

        return num
    end
end

local function AskForInteger(ctx: shellctx.ShellContext, prompt: string): number
    while true do 
        local num = AskForNumber(ctx, prompt)
        if num % 1 ~= 0 then 
            ctx:Log("Please enter an integer.")
            continue
        end

        return num
    end
end

local function Ask(ctx: shellctx.ShellContext, prompt: string): string
    while true do 
        local res = ctx:GetInput(prompt)
        if res == nil then error("Unexpected EOF") end

        return res
    end
end

local function createmodhelpers(ctx: shellctx.ShellContext) 
    local kvdb = ctx:CreateKvGod()

    --- Give permissions to a member in a guild or user context through a permission override
    local function GivePermsToMember(id: id.LuaId, userid: string, perms: {string}) 
        kvdb:set(id, userid, perms, {"builtins.guild_permissions.overrides"})
    end
    local function GivePermsToMemberInteractive() 
        local id = AskForLuaId(ctx)
        local userid = Ask(ctx, "User ID: ")
        local perms = AskForStringList(ctx, "Permissions to give (e.g. 'global.*', etc)")

        if not ConfirmChoice(ctx, `Give permissions {table.concat(perms, ",")} to user {userid} in {id.tenant_type} {id.tenant_id}?`) then
            ctx:Log("Aborting.")
            return
        end

        GivePermsToMember(id, userid, perms)
        ctx:Log(`Permissions {perms} given to user {userid} in {id.tenant_type} {id.tenant_id}`)
    end

    -- Set permissions for a role in a guild or user context through a permission override
    local function SetRolePermission(id: id.LuaId, roleid: string, index: number, perms: {string})
        kvdb:set(id, roleid, {
            index = index,
            perms = perms,
        }, {"builtins.guild_permissions"})
    end

    local function SetRolePermissionInteractive() 
        local id = AskForLuaId(ctx)
        local roleid = Ask(ctx, "Role ID: ")
        local index = AskForInteger(ctx, "Permission override index: ")
        local perms = AskForStringList(ctx, "Permissions to set (e.g. 'administrator', 'manage_messages', etc)")

        if not ConfirmChoice(ctx, `Set permissions {perms} for role {roleid} in {id.tenant_type} {id.tenant_id} at index {index}?`) then
            ctx:Log("Aborting.")
            return
        end

        SetRolePermission(id, roleid, index, perms)
        ctx:Log(`Permissions {perms} set for role {roleid} in {id.tenant_type} {id.tenant_id} at index {index}`)
    end

    return {
        -- GivePermsToMember
        GivePermsToMember = GivePermsToMember,
        GivePermsToMemberInteractive = GivePermsToMemberInteractive,

        -- SetRolePermission
        SetRolePermission = SetRolePermission,
        SetRolePermissionInteractive = SetRolePermissionInteractive,
    }
end

return createmodhelpers