local data = require"@antiraid-ext/frameworkv2/context"
local embed = require"../../auxutils/backups/embed"
local paginate = require "@antiraid-ext/frameworkv2/paginate"
local discord = require"@discord-types/apiTypes"
local units = require"@antiraid-ext/frameworkv2/unit"
local managers = require "../../auxutils/managers/managers"

--- Backup contents may only be read by users who can create or restore backups (matches the dashboard)
local function assertcanreadbackups(data: data.CommandContext)
    if not pcall(data.interaction.assertpermission, "backups.restore") then
        data.interaction.assertpermission("backups.create")
    end
end

local function list(data: data.CommandContext): nil
    assertcanreadbackups(data)

    local mdm = managers.getmanagers(data.ctx).backupmetadata

    local backups = mdm.listarr()

    if #backups == 0 then
        return data.interaction.replysimpleembed("No backups found", "There are no backups available for this server.", units.RED_COLOR)
    end

    paginate.paginate(data, {
        id = "backups",
        numPages = #backups,
        ephemeral = true, -- Embeds contain signed download links
        embed = function(currentIdx: number): discord.EmbedObject 
            local backupData = backups[currentIdx]
            local embed, _ = embed.toembed(
                data.ctx,
                {
                    key = backupData.key,
                    mdm = mdm,
                    createdat = backupData.createdat,
                },
                true
            )
            return embed
        end
    })
    return nil
end

return list