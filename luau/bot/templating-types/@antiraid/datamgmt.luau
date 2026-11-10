--- The files of a tar archive, mapping each path to its contents
export type TarFiles = { [string]: buffer | string }

--- Encodes a Blob to a base64 string
local function base64encode(data: buffer | string): string
    error("Implemented internally in AntiRaid runtime!")
end

--- Decodes a base64 string to a Blob
local function base64decode(str: string): buffer
    error("Implemented internally in AntiRaid runtime!")
end

--- Reads a tar archive into a table of path -> file contents
---
--- Only regular files are read. Errors on more than 64 entries or over 2 MiB of file data in total
local function loadtar(data: buffer | string): { [string]: buffer }
    error("Implemented internally in AntiRaid runtime!")
end

--- Builds a tar archive from a table of path -> file contents
local function maketar(files: TarFiles): buffer
    error("Implemented internally in AntiRaid runtime!")
end

--- Encrypts the Blob using AES256 encryption (Argon2id for key derivation)
--- Format: ``<salt><nonce><ciphertext>``
local function aes256encrypt(data: buffer | string, key: string): buffer
    error("Implemented internally in AntiRaid runtime!")
end 

--- Decrypts the Blob using AES256 decryption (Argon2id for key derivation)
--- Format: ``<salt><nonce><ciphertext>``
local function aes256decrypt(data: buffer | string, key: string): buffer
    error("Implemented internally in AntiRaid runtime!")
end

--- Compresses the given data with gzip using the specified compression level
local function compressgzip(data: buffer | string, level: number?): buffer
    error("Implemented internally in AntiRaid runtime!")
end

--- Decompresses the given data with gzip
local function decompressgzip(data: buffer | string): buffer
    error("Implemented internally in AntiRaid runtime!")
end

return {
    base64encode = base64encode,
    base64decode = base64decode,
    loadtar = loadtar,
    maketar = maketar,
    aes256encrypt = aes256encrypt,
    aes256decrypt = aes256decrypt,
    compressgzip = compressgzip,
    decompressgzip = decompressgzip,
}