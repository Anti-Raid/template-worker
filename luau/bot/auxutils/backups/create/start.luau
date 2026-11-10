local Primitives = require"@antiraid-core/primitives"
local datamgmt = require"@antiraid/datamgmt"
local coreBackup = require"./corebackup"
local types = require"../backups"
local json = require"@antiraid/json"
local finalize = require"./finalize"
local assetfetch = require"./assetfetch"
local conditions = require"../conditions"
local typesext = require"@antiraid/typesext"
local sb = require"@antiraid-ext/utils/statusbuffer"

local function createBackup(ctx: Primitives.TemplateContext, opts: types.BackupCreateOpts, enckey: string?, updater: sb.StatusBuffer): types.BuiltBackup
    conditions.validate(opts)
    local id = typesext.randstring(24)
    updater.addStatus("Current ID: " .. id)
    updater.addStatus("Collecting core backup data with perChannel of " .. opts.perChannel .. " and maxMessages of " .. opts.maxMessages)
    local coreBackup = coreBackup(ctx, opts, updater)
    updater.addStatus("Generating backup archive")
    -- datamgmt.maketar takes a table of path -> file data
    local tararchive: datamgmt.TarFiles = {}
    local encoded = json.tojsonstring(coreBackup)
    tararchive["core.json.gz"] = datamgmt.compressgzip(encoded)

    -- Add assets
    for _, asset in opts.backupGuildAssets do
        local assetBlob = assetfetch.fetchAsset(ctx, coreBackup.guild, asset)
        if assetBlob then
            tararchive[`assets/{asset}.jpg`] = assetBlob
        end
    end

    return finalize.buildBackup(id, tararchive, if enckey and #enckey > 0 then enckey else nil, updater)
end

return createBackup