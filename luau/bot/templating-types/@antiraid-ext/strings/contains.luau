--- Returns true if the string `str` contains the substring `sub`, false otherwise.
--- @param str string The string to search in.
--- @param sub string The substring to search for.
--- @return boolean ``true` if `str` contains `sub`, ``false`` otherwise.
local function string_contains(str: string, sub: string)
    return str:find(sub, 1, true) ~= nil
end

-- Test
if(_G.cli ~= nil) then
    assert(string_contains("Hello, world!", "world") == true, "string_contains failed")
    assert(string_contains("Hello, world!", "World") == false, "string_contains failed")
end

return string_contains