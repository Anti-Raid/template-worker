local commandBuilder = require "@discord-types/builders/interaction/interaction"
local kexs = require"../../auxutils/managers/managers"
local datetime = require "@antiraid/datetime"
local data = require"@antiraid-ext/frameworkv2/context"
local units = require "@antiraid-ext/frameworkv2/unit"

local command = commandBuilder.new({
    name = "remindme",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("Sends a specified message to a user after a specified amount of time")
:option(
    function(opt) 
        return opt
        :setType("String")
        :setName("message")
        :setDescription("The message to send")  
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

local function run(data: data.CommandContext): nil
    local user = data.interaction.user()
    local message: string? = data.args.string("message")
    local stime = data.args.time("time", "timeunit")
    local isResponseEphemeral = data.args.boolean("ephemeral") or false

    if not user or not message or not stime then
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

    assert(message, "message option is required") -- should never fail since we set it as required in the command definition

    local dt = datetime.UTC:now() + datetime.timedelta_seconds(stime or 1)
    kexs.getmanagers(data.ctx).remindmeexpiry.addat(dt, {
        userid = user.id,
        channelid = data.interaction.interaction.channel_id or error("Channel ID is nil"),
        message = message,
        ephemeral = isResponseEphemeral,
    })

    data.interaction.reply{
        embeds = {
            {
                title = "Created reminder",
                description = "You will be reminded at <t:" .. dt.timestamp_seconds .. ">",
                color = units.GREEN_COLOR
            },
        },
        ephemeral = isResponseEphemeral
    }
    return nil
end

return {
    command = command,
    run = run,
}