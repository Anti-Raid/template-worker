local commandBuilder = require "@discord-types/builders/interaction/interaction"
local context = require"@antiraid-ext/frameworkv2/context"
local datetime = require "@antiraid/datetime"
local managers = require "../../auxutils/managers/managers"
local units = require"@antiraid-ext/frameworkv2/unit"

local command = commandBuilder.new({
    name = "afk",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("Set yourself as AFK")
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("start")
        :setDescription("Start being AFK")  
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("reason")
                :setDescription("The message to send when users try to interact with you")  
                :setRequired(true)
                :build()      
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("Number")
                :setName("time")
                :setDescription("How long you want to be AFK for")  
                :setRequired(true)
                :build()      
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("timeunit")
                :setDescription("The unit of time")  
                :setRequired(true)
                :choice(function(choice) return choice:setName("Seconds"):setValue("seconds"):build() end)
                :choice(function(choice) return choice:setName("Minutes"):setValue("minutes"):build() end)
                :choice(function(choice) return choice:setName("Hours"):setValue("hours"):build() end)
                :choice(function(choice) return choice:setName("Days"):setValue("days"):build() end)
                :build()      
            end
        )
        :option(
            function(opt) 
                return opt
                :setType("Boolean")
                :setName("ephemeral")
                :setDescription("Whether to make the response ephemeral or not")
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
        :setName("stop")
        :setDescription("Stop being AFK")  
        :build()
    end
)
:build()

local function run(data: context.CommandContext): nil
    local cmdname = data.command.nameList[2]
    local afkmanager = managers.getmanagers(data.ctx).afkmanager

    if cmdname == "start" then
        local user = data.interaction.user()
        local reason = data.args.string("reason")
        local stime = data.args.time("time", "timeunit")
        local isResponseEphemeral = data.args.boolean("ephemeral")

        if not reason or not stime then
            data.interaction.reply{
                embeds = {
                    {
                        title = "Error processing command",
                        description = "Something went wrong while trying to find message and time. Please try again?",
                        color = units.RED_COLOR
                    },
                },
                ephemeral = isResponseEphemeral
            }
            return nil
        end

        afkmanager.set({
            user_id = user.id,
            reason = reason or "No reason provided",
            duration = datetime.timedelta_seconds(stime or 1),
        })

        data.interaction.reply{
            embeds = {
                {
                    title = "AFK Set",
                    description = "Enjoy the peace and quiet!",
                    color = units.GREEN_COLOR
                },
            },
            ephemeral = isResponseEphemeral
        }
        return nil
    elseif cmdname == "stop" then
        local user = data.interaction.user()
        local afk = afkmanager.get(user.id)
        if not afk then
            data.interaction.reply{
                embeds = {
                    {
                        title = "You are not AFK",
                        description = "You are not AFK, so you cannot stop being AFK.", 
                        color = units.RED_COLOR
                    },
                },
                ephemeral = true
            }
            return nil
        end

        afkmanager.delete(user.id)

        data.interaction.reply{
            embeds = {
                {
                    title = "AFK Stopped",
                    description = "You are no longer AFK. Welcome back!",
                    color = units.GREEN_COLOR
                },
            },
            ephemeral = true
        }
        
        return nil
    else 
        return data.interaction.replysimpleembed("Error processing command", "Unknown subcommand", units.RED_COLOR)
    end
end

return {
    command = command,
    run = run,
}