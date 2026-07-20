local Primitives = require("@antiraid-core/primitives")
local discord = require("@discord-types/apiTypes")
local createTab = require("@antiraid-ext/events/dispatch").createTab
 
--- GuildAuditLogEntryCreate
---
--- Dispatched when a new audit log entry is created.
---
--- Provides said entry’s data and the id of the guild where it was created.
local function GuildAuditLogEntryCreate(callback: (ctx: Primitives.TemplateContext, entry: discord.AuditLogEntryObject) -> ())
    return createTab("GUILD_AUDIT_LOG_ENTRY_CREATE", function(ctx, event)
        return callback(ctx, event.data)
    end)
end

return GuildAuditLogEntryCreate