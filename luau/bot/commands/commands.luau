local commands:  { [string]: any } = {
    afk = require"./afk/afk",
    backups = require"./backups/backups",
    help = require"./help/help",
    honeypot = require"./honeypot/honeypot",
    lockdowns = require"./lockdowns/lockdowns",
    moderation = require"./moderation/moderation",
    perms = require"./perms/perms",
    remindme = require"./remindme/remindme",
    serverinfo = require"./serverinfo/serverinfo",
    stats = require"./stats/stats",
    stings = require"./stings/stings",
    whois = require"./whois/whois",
    auditlogs = require"./auditlogs/auditlogs"
}

return {
    commands = commands,
    b = 3
}
