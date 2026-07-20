--- Returns the character at the specified index in the string.
--- @param str string The string to index.
--- @param idx number The index of the character to retrieve (1-based).
--- @return string The character at the specified index.
local function string_index(str: string, idx: number) --> string
    return str:sub(idx, idx)
end

return string_index