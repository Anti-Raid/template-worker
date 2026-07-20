local commandBuilder = require "@discord-types/builders/interaction/interaction"
local data = require"@antiraid-ext/frameworkv2/context"

local createcmd = require "./create"
local restorecmd = require "./restore"
local listcmd = require "./list"
local deletecmd = require "./delete"
local inspectcmd = require "./inspect"

local command = commandBuilder.new({
    name = "backups",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("Manage and create backups of your server")
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("create")
        :setDescription("Creates a new backup of the server")  
        :option(
            function(opt) 
                return opt
                :setType("Boolean")
                :setName("messages")
                :setDescription("Whether to include messages in the backup (up to 500)")
                :setRequired(false)  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("channels")
                :setDescription("Channels to backup messages from (comma separated), otherwise all channels")  
                :setRequired(false)
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("backup_guild_assets")
                :setDescription("What assets to back up in comma-seperated form (icon,splash,banner)")
                :setRequired(false)  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("Integer")
                :setName("max_messages")
                :setDescription("The maximum number of messages to backup. Defaults to 500")
                :setRequired(false)  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("Integer")
                :setName("per_channel")
                :setDescription("The number of messages per channel to backup. Defaults to 100")
                :setRequired(false)  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("special_allocations")
                :setDescription("Specific channel allocation overrides. Format: channel_id=number,channel_id=number")
                :setRequired(false)  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("password")
                :setDescription("Password to encrypt the backup with. If not provided, the backup will not be encrypted")
                :setRequired(false)  
                :build()
            end
        )
        :build()      
    end
)
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("restore")
        :setDescription("Restores a backup to the server")
        :option(
            function(opt) 
                return opt
                :setType("Attachment")
                :setName("backup_file")
                :setDescription("The backup attachment to restore")
                :setRequired(false)  
                :build()
            end   
        ) 
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("backup_id")
                :setDescription("The backup ID to restore from")
                :setRequired(false)  
                :build()
            end   
        ) 
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("password")
                :setDescription("Password to decrypt backup with. Should not be reused")
                :setRequired(false)  
                :build()
            end   
        ) 
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("protected_channels")
                :setDescription("Channels to protect from being deleted. Comma separated")
                :setRequired(false)  
                :build()
            end   
        )
        :option(
            function(opt)
                return opt
                :setType("String")
                :setName("protected_roles")
                :setDescription("Roles to protect from being deleted. Comma separated")
                :setRequired(false)  
                :build()
            end
        )
        :build()
    end
)
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("inspect")
        :setDescription("Inspect a created backup to see past memories")
        :option(
            function(opt) 
                return opt
                :setType("Attachment")
                :setName("backup_file")
                :setDescription("The backup attachment to inspect")
                :setRequired(false)  
                :build()
            end   
        ) 
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("backup_id")
                :setDescription("The backup ID to inspect")
                :setRequired(false)  
                :build()
            end   
        ) 
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("password")
                :setDescription("Password to decrypt backup with. Should not be reused")
                :setRequired(false)  
                :build()
            end   
        ) 
        :build()
    end
)
:option(
    function(opt)
        return opt
        :setType("SubCommand")
        :setName("list")
        :setDescription("Lists all backups of the server")
        :build()
    end
)
:option(
    function(opt)
        return opt
        :setType("SubCommand")
        :setName("delete")
        :setDescription("Deletes a backup by its ID")
        :option(
            function(opt)
                return opt
                :setType("String")
                :setName("backup_id")
                :setDescription("The backup ID to delete")
                :setRequired(true)
                :build()
            end
        )
        :build()
    end
)
:build()

local function run(data: data.CommandContext): nil
    local cmdname = data.command.nameList[2]

    if cmdname == "create" then
        return createcmd(data)
    elseif cmdname == "restore" then
        return restorecmd(data)
    elseif cmdname == "list" then
        return listcmd(data)
    elseif cmdname == "delete" then
        return deletecmd(data)
    elseif cmdname == "inspect" then
        return inspectcmd(data)
    else
        error("Unknown subcommand")
    end
end

return {
    command = command,
    run = run,
}
