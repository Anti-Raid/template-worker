--- Returns ``true`` if the string `str` ends with the substring `ending`, ``false`` otherwise.
--- @param str string The string to check.
--- @param ending string The substring to check for at the end of `str`.
--- @return boolean ``true`` if `str` ends with `ending`, ``false`` otherwise.
local function string_endswith(str: string, ending: string)
    return ending == "" or str:sub(-#ending) == ending
end

-- Test
if(_G.cli ~= nil) then
    assert(string_endswith("Hello, world!", "world!") == true, "string_endswith failed")
    assert(string_endswith("Hello, world!", "World!") == false, "string_endswith failed")
    assert(string_endswith("!", "World!") == false, "!string_endswith failed")
    assert(string_endswith("!", "!") == true, "!string_endswith failed")
end


return string_endswith