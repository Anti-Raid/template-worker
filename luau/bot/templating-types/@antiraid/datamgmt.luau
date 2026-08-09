export type TarArchive = {
    --- Takes out an entry from the tar archive by file name returning nil if not found
    takefile: (self: TarArchive, name: string) -> buffer?,
    --- Adds an entry to the tar archive with the given file name and contents
    addfile: (self: TarArchive, name: string, contents: buffer | string) -> nil,
    --- Returns the names of all entries in the tar archive
    entries: (self: TarArchive) -> { string },
    --- Converts the tar archive to a Blob
    ---
    --- This will destroy the tar archive (hence making it unusable for future Luau operations) 
    --- and return a Blob containing the tar archive data
    toblob: (self: TarArchive) -> buffer,
}

--- Encodes a Blob to a base64 string
local function base64encode(data: buffer | string): string
    error("Implemented internally in AntiRaid runtime!")
end

--- Decodes a base64 string to a Blob
local function base64decode(str: string): buffer
    error("Implemented internally in AntiRaid runtime!")
end

--- Creates a new TarArchive with an optional initial data Blob to load the initial TarArchive's contents from
local function TarArchive(buf: (buffer | string)?): TarArchive
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
    TarArchive = TarArchive,
    aes256encrypt = aes256encrypt,
    aes256decrypt = aes256decrypt,
    compressgzip = compressgzip,
    decompressgzip = decompressgzip,
}