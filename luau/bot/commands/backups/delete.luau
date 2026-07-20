local data = require"@antiraid-ext/frameworkv2/context"
local units = require"@antiraid-ext/frameworkv2/unit"
local managers = require "../../auxutils/managers/managers"

local function delete(data: data.CommandContext): nil
    data.interaction.assertpermission("backups.delete")
    local mdm = managers.getmanagers(data.ctx).backupmetadata

    local backupId = nil
    for _, opt in data.command.options do
        if opt.name == "backup_id" and opt.value.kind == "String" then
            backupId = opt.value.value
        end
    end
    if not backupId then
        return data.interaction.replysimpleembed("Error processing command", "You must specify a backup ID to delete.", units.RED_COLOR)
    end

    -- Directly call delete, rely on global xpcall for error handling
    local backupData = mdm.get(backupId)

    if not backupData then
        return data.interaction.replysimpleembed("Error processing command", "Backup with ID `" .. backupId .. "` does not exist.", units.RED_COLOR)
    end
    
    mdm.remove(backupId)
    return data.interaction.replysimpleembed("Backup Deleted", string.format("Backup with ID `%s` has been deleted.", backupId), units.GREEN_COLOR)
end

return delete