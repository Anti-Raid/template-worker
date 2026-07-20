--- Strips a set of characters from the beginning and end of a string.
---
--- @param str string The string to strip.
--- @param chars string? The characters to strip. Defaults to whitespace characters (" \t\n\r").
--- @return string The stripped string.
--- @return number The index of the last character in the stripped string.
local function string_strip(str: string, chars: string?): (string, number)
    local start = 1
    local finish = #str
    chars = chars or " \t\n\r"
    local str_len = #str

    while start <= str_len and chars:find(str:sub(start, start), 1, true) do
        start = start + 1
    end

    while finish >= start and chars:find(str:sub(finish, finish), 1, true) do
        finish = finish - 1
    end

    return str:sub(start, finish), finish
end

-- Test
if(_G._TEST_ASYNC_WORK ~= nil) then
    assert(string_strip("  Hello, world!  ") == "Hello, world!", "string_strip failed [1]")
    assert(string_strip("  Hello, world!  ", "Hello") == "  Hello, world!  ", "string_strip failed [2]")
    assert(string_strip("  Hello, world!  ", "Hello ") == ", world!", "string_strip failed [3]")
    assert(string_strip("Hello world ", "d ") == "Hello worl", "string_strip failed [4]")
end

return string_strip