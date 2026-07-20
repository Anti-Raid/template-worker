local commandBuilder = require "@discord-types/builders/interaction/interaction"
local data = require"@antiraid-ext/frameworkv2/context"
local datetime = require("@antiraid/datetime")
local formatTimeDelta = require "@antiraid-ext/formattimedelta"

local command = commandBuilder.new({
    name = "stats",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("View stats about the bot")
:build()

local function run(data: data.CommandContext)
    local stats = data.framework.meta.stats()

    local startedAt = datetime.UTC:now():duration_since(stats.last_started_at)

    data.interaction.reply{
        embeds = {
            {
                title = "Stats",
                description = "Here are the some cool bot stats",
                fields = {
                    { name = "Bot Name", value = tostring(data.ctx.btd().bot.username), inline = true },
                    { name = "Total Users", value = tostring(stats.total_users), inline = true },
                    { name = "Total Servers", value = tostring(stats.total_guilds), inline = true },
                    { name = "Bot Last Started", value = "<t:" .. stats.last_started_at.timestamp_seconds .. ">", inline = true },
                    { name = "Bot Uptime", value = formatTimeDelta(startedAt), inline = true }
                }
            }
        }
    }
end

return {
    command = command,
    run = run,
}