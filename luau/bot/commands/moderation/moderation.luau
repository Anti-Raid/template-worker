local commandBuilder = require "@discord-types/builders/interaction/interaction"
local data = require"@antiraid-ext/frameworkv2/context"
local ban = require "./ban"
local unban = require "./unban"
local kick = require "./kick"
local units = require"@antiraid-ext/frameworkv2/unit"

local command = commandBuilder.new({
    name = "moderation",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("Moderation related commands")
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("ban")
        :setDescription("Ban a user from the server")  
        :option(
            function(opt) 
                return opt
                :setType("User")
                :setName("user")
                :setDescription("User to ban")
                :setRequired(true)  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("reason")
                :setDescription("Reason for the ban")  
                :setRequired(true)
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("Integer")
                :setName("delete_message_seconds")
                :setDescription("Number of seconds to prune messages for. Mutually exclusive with delete_message_days")  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("Number")
                :setName("duration")
                :setDescription("How long should the user be banned for. Must also provide durationunit")
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("durationunit")
                :setDescription("The unit for the duration to ban for")  
                :choice(function(choice) return choice:setName("Seconds"):setValue("seconds"):build() end)
                :choice(function(choice) return choice:setName("Minutes"):setValue("minutes"):build() end)
                :choice(function(choice) return choice:setName("Hours"):setValue("hours"):build() end)
                :choice(function(choice) return choice:setName("Days"):setValue("days"):build() end)
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
        :setName("unban")
        :setDescription("Unbans a user from the server")  
        :option(
            function(opt) 
                return opt
                :setType("User")
                :setName("user")
                :setDescription("User to unban")
                :setRequired(true)  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("reason")
                :setDescription("Reason for the ban")  
                :setRequired(true)
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
        :setName("kick")
        :setDescription("Kick a user from the server")  
        :option(
            function(opt) 
                return opt
                :setType("User")
                :setName("user")
                :setDescription("User to kick")
                :setRequired(true)  
                :build()
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("reason")
                :setDescription("Reason for kicking the user")  
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

    if cmdname == "ban" then return ban(data) 
    elseif cmdname == "unban" then return unban(data)
    elseif cmdname == "kick" then return kick(data)
    else return data.interaction.replysimpleembed("Error processing command", "Unknown subcommand", units.RED_COLOR)
    end
end

return {
    command = command,
    run = run,
}