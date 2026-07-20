local data = require"@antiraid-ext/frameworkv2/context"
local backupTypes = require"../../auxutils/backups/backups"
local units = require"@antiraid-ext/frameworkv2/unit"
local sb = require"@antiraid-ext/utils/statusbuffer"
local tempbutton = require"@antiraid-ext/frameworkv2/tempbutton"
local managers = require "../../auxutils/managers/managers"

local function restore(data: data.CommandContext): nil
    data.interaction.assertpermission("backups.restore")
    local managers = managers.getmanagers(data.ctx)

    local opts: backupTypes.BackupRestoreOpts = {
        protectedChannels = {},
        protectedRoles = {},
    }
    local enckey: string? = nil
    local source: backupTypes.RestoreSource? = nil

    if data.interaction.interaction.channel_id then 
        opts.protectedChannels = {data.interaction.interaction.channel_id}
    end

    for _, opt in data.command.options do 
        if opt.name == "backup_file" then
            if opt.value.kind == "Attachment" then
                if source then 
                    return data.interaction.replysimpleembed("Error processing command", "You cannot specify both a backup file and a backup ID. Please choose one.", units.RED_COLOR)
                end
                source = {
                    type = "attachment" :: "attachment",
                    url = opt.value.attachment.url or error("No valid URL found for the backup file"),
                    filename = opt.value.attachment.filename or error("No valid filename found for the backup file"),
                }
            end
        elseif opt.name == "backup_id" then
            if opt.value.kind == "String" then
                if source then
                    return data.interaction.replysimpleembed("Error processing command", "You cannot specify both a backup file and a backup ID. Please choose one.", units.RED_COLOR)
                end

                source = {
                    type = "kv" :: "kv",
                    key = opt.value.value,
                }
            end
        elseif opt.name == "password" then
            if opt.value.kind == "String" then
                enckey = opt.value.value
            end
        elseif opt.name == "protected_channels" then
            if opt.value.kind == "String" then 
                local pc = string.split(opt.value.value, ",")
                for _, channelId in pc do
                    -- remove any whitespace
                    channelId = channelId:gsub("%s+", "")
                    if channelId ~= "" then
                        table.insert(opts.protectedChannels, channelId)
                    end
                end
            end
        elseif opt.name == "protected_roles" then
            if opt.value.kind == "String" then 
                local pr = string.split(opt.value.value, ",")
                for _, roleId in pr do
                    -- remove any whitespace
                    roleId = roleId:gsub("%s+", "")
                    if roleId ~= "" then
                        table.insert(opts.protectedRoles, roleId)
                    end
                end
            end
        end
    end

    -- Check if we have a backup source
    if not source then
        return data.interaction.replysimpleembed("Error processing command", "You must specify either a backup file or a backup ID to restore from.", units.RED_COLOR)
    end

    -- Send spawning message
    local buf = sb.StatusBuffer()
    local tempbutton = tempbutton.tempbutton(data, {
        title = "View Logs",
        embed = function() 
            return {
                title = "Logs",
                description = table.concat(sb.stringifybuffer(buf, "Discord"), "\n"),
                color = units.YELLOW_COLOR
            }
        end
    })
    data.interaction.replysimpleembed("Restoring backup", "Please wait...", units.GREEN_COLOR, {tempbutton.actionrow})
    managers.backupcheckpointmanager.new({
        opts = opts,
        enckey = enckey,
        gre = {
            restoredRoles = {},
            restoredChannels = {},
        },
        src = source
    }, buf)

    data.interaction.replyedit({
        embeds = {
            {
                title = "Backup Restored Successfully",
                color = units.GREEN_COLOR
            }
        }
    })

    return nil
end

return restore