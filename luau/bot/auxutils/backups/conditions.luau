local types = require"./backups"

local MAX_MESSAGES = 100 -- Maximum number of messages to backup totally (100 is a sweet spot between usability and time needed to create and restore backups)
local MAX_PER_CHANNEL = 100 -- Maximum number of messages to backup per channel (50 is a sweet spot between usability and time needed to create and restore backups)
local MIN_PER_CHANNEL = 20 -- Minimum number of messages to backup per channel (5 is the minimum)
local MAX_STRIP_ITERS = 300

--- Validates the backup options to ensure they are within the allowed limits.
--- This function will throw an error if the options are invalid.
--- @param data The backup options to validate
--- @return nil if the options are valid, otherwise an error is thrown
local function validate(data: types.BackupCreateOpts) 
    if data.maxMessages < 1 or data.maxMessages > MAX_MESSAGES then
        error(`maxMessages must be between 1 and {MAX_MESSAGES}, got {data.maxMessages}`)
    end

    if data.perChannel < MIN_PER_CHANNEL or data.perChannel > MAX_PER_CHANNEL then
        error(`perChannel must be between {MIN_PER_CHANNEL} and {MAX_PER_CHANNEL}, got {data.perChannel}`)
    end

    if data.perChannel > data.maxMessages then
        error(`perChannel cannot be greater than maxMessages, got perChannel={data.perChannel} and maxMessages={data.maxMessages}`)
    end
end

return {
    MAX_MESSAGES = MAX_MESSAGES,
    MAX_PER_CHANNEL = MAX_PER_CHANNEL,
    MIN_PER_CHANNEL = MIN_PER_CHANNEL,
    MAX_STRIP_ITERS = MAX_STRIP_ITERS,
    validate = validate
}