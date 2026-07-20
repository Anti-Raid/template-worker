local datamgmt = require"@antiraid/datamgmt"
local datetime = require"@antiraid/datetime"
local types = require"../backups"
local sb = require"@antiraid-ext/utils/statusbuffer"

--- Builds a backup from a TAR archive and an optional encryption key.
--- If the encryption key is provided, the backup will be encrypted using AES256.
--- The backup will be returned as a `BuiltBackup` object containing the backup data and the filename.
--- @param tar The TAR archive to build the backup from
--- @param enckey The optional encryption key to encrypt the backup with
--- @return A `BuiltBackup` object containing the backup data and the filename
local function buildBackup(
    id: string,
    tar: datamgmt.TarArchive,
    enckey: string?,
    updater: sb.StatusBuffer
): types.BuiltBackup
    local filename = `antiraidbackup-{datetime.UTC:now()}_{id}`
    local fileext = ".arb2"
    local data = tar:toblob()
    if enckey then 
        data = datamgmt.aes256encrypt(data, enckey)
        fileext = ".arb2e"
    end

    updater.addStatus("Finalized archive")

    return {
        id = id,
        backup = data,
        filename = filename .. fileext,
    }
end

return {
    buildBackup = buildBackup,
}