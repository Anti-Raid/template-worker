local Message = require "@antiraid-ext/events/discord/Message"
local Custom = require "@antiraid-ext/events/antiraid/Custom"
local commands = require "./commands/commands"
local settings = require "./settings/settings"
local afkhandler = require"./auxutils/afkhandler"
local honeypothandler = require"./auxutils/honeypothandler"
local managers = require"./auxutils/managers/managers"
local Framework = require"@antiraid-ext/frameworkv2"

-- Audit log event handlers
local auditlogBan = require"./auxutils/auditlogs/AntiraidBan"
local auditlogKick = require"./auxutils/auditlogs/AntiraidKick"
local auditlogUnban = require"./auxutils/auditlogs/AntiraidUnban"
local auditlogStingCreated = require"./auxutils/auditlogs/BuiltinsStingCreated"
local auditlogStingDelete = require"./auxutils/auditlogs/BuiltinsStingDelete"

return Framework.setup(
    commands.commands,
    settings,
    Message(function(ctx, msg)
        managers.getmanagers(ctx) -- Ensure managers are initialized before handling any messages, it is generally a good idea to ensure all managers are ready before processing any events
        afkhandler(msg, ctx)
        honeypothandler(msg, ctx)
    end),
    Custom("OnStartup")(function(ctx, data)
        -- Initialize afk/sting/key expiration managers on startup
        -- along with backup restore checkpoint expiry with oninit logic to resume existing checkpoints
        managers.getmanagers(ctx)
    end),
    -- Audit log event handlers
    auditlogBan,
    auditlogKick,
    auditlogUnban,
    auditlogStingCreated,
    auditlogStingDelete
)
