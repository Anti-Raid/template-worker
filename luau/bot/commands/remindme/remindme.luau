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

--- Limits on reminders
---
--- All reminders of a guild are kept in memory and reloaded in a single request on startup,
--- so both their number and their size must stay bounded
local MAX_MESSAGE_LENGTH = 1000
local MAX_REMINDER_SECONDS = 365 * 24 * 60 * 60 -- 1 year
local MAX_REMINDERS_PER_USER = 25
local MAX_REMINDERS_PER_GUILD = 1000

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

    local function replylimiterror(description: string): nil
        data.interaction.reply{
            embeds = {
                {
                    title = "Cannot create reminder",
                    description = description,
                    color = units.RED_COLOR
                },
            },
            ephemeral = isResponseEphemeral
        }
        return nil
    end

    if #message > MAX_MESSAGE_LENGTH then
        return replylimiterror(`Reminder messages can be at most {MAX_MESSAGE_LENGTH} characters long.`)
    end

    if stime <= 0 or stime > MAX_REMINDER_SECONDS then
        return replylimiterror("Reminders must be set for a time in the future that is at most 1 year away.")
    end

    local remindmeexpiry = kexs.getmanagers(data.ctx).remindmeexpiry

    local guildCount, userCount = 0, 0
    for _, record in remindmeexpiry.list() do
        guildCount += 1
        if record.value.userid == user.id then
            userCount += 1
        end
    end

    if userCount >= MAX_REMINDERS_PER_USER then
        return replylimiterror(`You can have at most {MAX_REMINDERS_PER_USER} active reminders in this server.`)
    end

    if guildCount >= MAX_REMINDERS_PER_GUILD then
        return replylimiterror(`This server has reached the maximum of {MAX_REMINDERS_PER_GUILD} active reminders.`)
    end

    local dt = datetime.UTC:now() + datetime.timedelta_seconds(stime)
    remindmeexpiry.addat(dt, {
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