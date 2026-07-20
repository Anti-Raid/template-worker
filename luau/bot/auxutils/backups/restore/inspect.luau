local Primitives = require"@antiraid-core/primitives"
local types = require"../backups"
local load = require"./load"
local compare = require"@antiraid-ext/utils/rolecmp"
local sb = require"@antiraid-ext/utils/statusbuffer"
local conditions = require"../conditions"

--- Partial restore manager
export type BackupInspector = {
    --- Returns the loaded backup
    getLoadedBackup: () -> types.LoadedBackup,
}

--- backup inspector
local function BackupInspector(
    ctx: Primitives.TemplateContext, 
    src: types.RestoreSource, 
    enckey: string?, 
    updater: sb.StatusBuffer
): BackupInspector
    local loadedBackup = load.loadBackupFile(ctx, src, enckey, updater)
    local coreData = loadedBackup.core
    assert(coreData.guild.roles, "invalid backup: guild roles data is missing in core data")
    compare.SortRolesByHierarchyDesc(coreData.guild.roles) -- Sort backup roles in hierarchy order
    
    conditions.validate(coreData.options)

    -- Set src and target guild ids and do basic validation
    local srcGuildId = coreData.guild.id
    assert(srcGuildId and #srcGuildId > 0, "invalid backup: guild ID is missing or invalid")
    assert(ctx.btd().id.tenant_type == "guild", "backups can only be used within a guild context")
    local targetGuildId = ctx.btd().id.tenant_id
    assert(targetGuildId and #targetGuildId > 0, "invalid context: guild ID is missing or invalid")

    local function getLoadedBackup()
        return loadedBackup
    end

    local self = {}
    self.getLoadedBackup = getLoadedBackup
    return self
end

return BackupInspector