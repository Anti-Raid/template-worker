local discord = require "@discord-types/apiTypes"
local commandBuilder = require "@discord-types/builders/interaction/interaction"
local data = require"@antiraid-ext/frameworkv2/context"
local units = require"@antiraid-ext/frameworkv2/unit"
local managers = require "../../auxutils/managers/managers"
local paginate = require "@antiraid-ext/frameworkv2/paginate"

local command = commandBuilder.new({
    name = "honeypot",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("Setup honeypot channels")
:option(
    function(opt)
        return opt
        :setType("SubCommand")
        :setName("list")
        :setDescription("List all honeypot channels")
        :build()  
    end     
)
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("setup")
        :setDescription("Sets up a channel as a honeypot")  
        :option(
            function(opt) 
                return opt
                :setType("Channel")
                :setName("channelid")
                :setDescription("The channel to set as a honeypot")  
                :setRequired(true)
                :build()      
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("Boolean")
                :setName("botallowed")
                :setDescription("Whether bots are allowed to message in this honeypot")  
                :setRequired(true)
                :build()      
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("punishment")
                :setDescription("What punishment to apply (ban/kick)")
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
        :setName("remove")
        :setDescription("Remove a honeypot from a channel")
        :option(
            function(opt) 
                return opt
                :setType("Channel")
                :setName("channelid")
                :setDescription("The honeypot channel to remove")  
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
    local honeypotmanager = managers.getmanagers(data.ctx).honeypotmanager

    if cmdname == "list" then
        data.interaction.assertpermission("honeypots.list")

        local honeypots = honeypotmanager.list()
        if #honeypots == 0 then
            return data.interaction.replysimpleembed("No Honeypots", "There are currently no honeypot channels set up.", units.YELLOW_COLOR)
        end

        local honeypotEmbeds: {discord.EmbedObject} = {}
        for _, honeypot in honeypots do
            table.insert(honeypotEmbeds, {
                title = "Honeypot Channel",
                description = string.format(
                    "Channel ID: `%s`\nBots Allowed: `%s`\nPunishment: `%s`\nCreated At: `%s`",
                    honeypot.channelid,
                    tostring(honeypot.botallowed),
                    honeypot.punishment,
                    tostring(honeypot.created_at)
                ),
                color = 0x00ffff, -- Cyan color
            } :: discord.EmbedObject)
        end

        paginate.paginate(data, {
            id = "honeypot_list",
            numPages = #honeypotEmbeds,
            embed = function(currentIdx: number): discord.EmbedObject 
                return honeypotEmbeds[currentIdx]
            end
        })
        return nil    
    elseif cmdname == "add" then
        data.interaction.assertpermission("honeypots.add")

        local channelid = data.args.channelid("channelid")
        local botallowed = data.args.boolean("botallowed")
        local punishment = data.args.string("punishment")

        -- TODO: Add more options here
        if punishment ~= "ban" and punishment ~= "kick" then
            return data.interaction.replysimpleembed("Invalid punishment", "Punishment must be either 'ban' or 'kick'.", units.RED_COLOR)
        end
        
        if not channelid or not botallowed or not punishment then
            return data.interaction.replysimpleembed("Error processing command", "Something went wrong while trying to find options. Please try again?", units.RED_COLOR) 
        end
        
        honeypotmanager.set({
            channelid = channelid,
            botallowed = botallowed,
            punishment = punishment,
        })

        return data.interaction.replysimpleembed("Honeypot Added/Updated", "Honeypot successfully added/updated for the specified channel.", units.GREEN_COLOR)
    elseif cmdname == "remove" then
        data.interaction.assertpermission("honeypots.remove")
        local channelid = data.args.channelid("channelid")

        if not channelid then
            return data.interaction.replysimpleembed("Error processing command", "Something went wrong while trying to find channel id. Please try again?", units.RED_COLOR)
        end

        if not honeypotmanager.has(channelid) then 
            return data.interaction.replysimpleembed("Honeypot Not Found", "No honeypot found for the specified channel.", units.RED_COLOR)
        end

        honeypotmanager.delete(channelid)
        return data.interaction.replysimpleembed("Honeypot Removed", "Honeypot successfully removed from the specified channel.", units.RED_COLOR)
    else 
        error("Unknown subcommand: " .. tostring(cmdname))
    end
end

return {
    command = command,
    run = run,
}