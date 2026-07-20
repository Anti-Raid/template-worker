local data = require"@antiraid-ext/frameworkv2/context"
local units = require"@antiraid-ext/frameworkv2/unit"
local backupTypes = require"../../auxutils/backups/backups"
local typesext = require"@antiraid/typesext"
local datetime = require"@antiraid/datetime"
local sb = require"@antiraid-ext/utils/statusbuffer"
local tempbutton = require"@antiraid-ext/frameworkv2/tempbutton"
local inspectimpl = require"../../auxutils/backups/restore/inspect"

local EMBED_EXPIRY = datetime.timedelta_minutes(5)

local function inspect(data: data.CommandContext): nil
    local enckey: string? = nil
    local source: backupTypes.RestoreSource? = nil

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
        end
    end

    -- Check if we have a backup source
    if not source then
        return data.interaction.replysimpleembed("Error processing command", "You must specify either a backup file or a backup ID to inspect.", units.RED_COLOR)
    end

    local inspectCustomId = `backups.inspect:${typesext.randstring(16)}`
    data.framework.components.addwithexpiry(inspectCustomId, function(data: data.MessageComponentContext): nil 
        data.interaction.reply{
            ephemeral = true,
            embeds = {
                {
                    title = "You clicked me!",
                    description = "...",
                },
            }
        }
        return nil
    end, EMBED_EXPIRY)

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
    data.interaction.replysimpleembed("Loading backup", "Please wait...", units.GREEN_COLOR, {tempbutton.actionrow})
    local loaded = inspectimpl(data.ctx, source, enckey, buf).getLoadedBackup()
    data.interaction.replyedit({
        embeds = {
            {
                title = `Backup of \`{loaded.core.guild.name}\``,
                description = table.concat({
                    "**Backed up server info**",
                    `Roles: \`{if loaded.core.guild.roles then #loaded.core.guild.roles else 0}\``,
                    `Channels: \`{#loaded.core.channels}\``,
                }, "\n"),
                color = units.GREEN_COLOR
            }
        }
    })
    return nil
end

return inspect