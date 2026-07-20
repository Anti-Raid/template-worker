--- Returns a new string with `text` inserted at position `pos` in `str`.
--- @param str string The original string.
--- @param pos number The position at which to insert `text` (1-based index).
--- @param text string The text to insert.
--- @return string The new string with `text` inserted at the specified position.
local function string_insert(str: string, pos: number, text: string)
    return str:sub(1, pos - 1) .. text .. str:sub(pos)
end

-- Test
if(_G.cli ~= nil) then
    assert(string_insert("Hello, world!", 8, "beautiful ") == "Hello, beautiful world!", "string_insert failed")
end

return string_insert