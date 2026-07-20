--!strict
local commandBuilder = require "@discord-types/builders/interaction/interaction"
local data = require"@antiraid-ext/frameworkv2/context"
local verifyMemberPermOverrideChange = require"../../settings/permvalidator".verifyMemberPermOverrideChange
local paginate = require "@antiraid-ext/frameworkv2/paginate"
local apitypes = require "@discord-types/apiTypes"
local units = require"@antiraid-ext/frameworkv2/unit"

local command = commandBuilder.new({
    name = "perms",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("Manage user permissions on the server.")
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("listmembers")
        :setDescription("List all current member permission overrides")  
        :build()      
    end
)
:build()

local function run(data: data.CommandContext): nil
    local cmdname = data.command.nameList[2]

    if cmdname == "listmembers" then
        verifyMemberPermOverrideChange({op = "View"})

        local memberPerms = data.framework.userinfomanager.getMemberPermissionOverrides()

        if #memberPerms == 0 then 
            return data.interaction.replysimpleembed("No Member Permission Overrides Found", "This server has no members with custom permission overrides set.", units.RED_COLOR)
        end

        local embeds = {}
        for _, memberPermissions in memberPerms do             
            local permsMap: {string} = {} 
            for _, _perm in memberPermissions.perm_overrides do 
                table.insert(permsMap, tostring(_perm))
            end
            
            local createdAtTs = if memberPermissions.createdat then "<t:" .. memberPermissions.createdat.timestamp_seconds .. ":f>" else "Default Value"
            table.insert(embeds, {
                title = "User ID: " .. memberPermissions.userid,
                description = "Permissions: " .. (if #permsMap > 0 then table.concat(permsMap, ", ") else "None") .. "\nCreated At: " .. createdAtTs .. "\nUser Mention: <@" .. memberPermissions.userid .. ">",
            } :: apitypes.EmbedObject)
        end

        local function renderPage(page: number)
            return embeds[page]
        end

        paginate.paginate(data, {
            numPages = #embeds,
            id = "perms.listmembers",
            embed = renderPage
        })

        return nil
    else 
        error("Unknown subcommand: " .. tostring(cmdname))
    end
end

return {
    command = command,
    run = run,
}