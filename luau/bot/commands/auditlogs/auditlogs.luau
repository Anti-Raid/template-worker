local commandBuilder = require "@discord-types/builders/interaction/interaction"
local data = require"@antiraid-ext/frameworkv2/context"
local units = require"@antiraid-ext/frameworkv2/unit"
local AuditLogManager = require "../../auxutils/auditlogmanager"
local strip = require"@antiraid-ext/strings/strip"
local managers = require "../../auxutils/managers/managers"

local command = commandBuilder.new({
    name = "auditlogs",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("Setup audit log channels")
:option(
    function(opt)
        return opt
        :setType("SubCommand")
        :setName("set")
        :setDescription("Set up an audit log for a channel")
        :option(
            function(opt)
                return opt
                :setType("Channel")
                :setName("channelid")
                :setDescription("The channel to send audit logs to")
                :setRequired(true)
                :build()
            end
        )
        :option(
            function(opt)
                return opt
                :setType("String")
                :setName("actions")
                :setDescription("Comma-separated list of actions to log (e.g. antiraid_ban,antiraid_kick)")
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
    local auditlogmanager = managers.getmanagers(data.ctx).auditlogmanager

    if cmdname == "set" then
        data.interaction.assertpermission("auditlog.set")

        local channelid = data.args.channelid("channelid")
        local actionsStr = data.args.string("actions")

        if not channelid or not actionsStr then
            return data.interaction.replysimpleembed("Error processing command", "Something went wrong while trying to find options. Please try again?", units.RED_COLOR)
        end

        -- Parse comma-separated actions
        local whats: {AuditLogManager.What} = {}
        for _, action in string.split(actionsStr, ",") do
            action, _ = strip(action)
            table.insert(whats, action :: AuditLogManager.What)
        end

        auditlogmanager.set({
            channel_id = channelid,
            whats = whats,
        })

        return data.interaction.replysimpleembed("Audit Log Set", "Audit log configuration successfully set for the specified channel.", units.GREEN_COLOR)
    else
        error("Unknown subcommand: " .. tostring(cmdname))
    end
end

return {
    command = command,
    run = run,
}