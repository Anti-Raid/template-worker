--- Splits a string into a table of substrings, using a separator string.
--- Stops splitting after n substrings have been found or the end of the string is reached.
---
--- @param str string The string to split.
--- @param sep string The separator string to use for splitting.
--- @param n number The maximum number of substrings to return.
--- @return A table of strings containing the substrings.
local function string_splitn(str: string, sep: string, n: number): {string}
    local parts = {}
    local i = 1

    while i <= n do
        local start_idx, end_idx = str:find(sep, 1, true)
        if (not start_idx or not end_idx) then
            break
        end

        table.insert(parts, str:sub(1, start_idx - 1))
        str = str:sub(end_idx + 1)
        i = i + 1
    end

    table.insert(parts, str)
    return parts
end

-- Test
if(_G._TEST_ASYNC_WORK ~= nil) then
    assert(string_splitn("Hello, world!", ",", 1)[1] == "Hello", "string_splitn failed., got " .. string_splitn("Hello, world!", ",", 1)[1])
    assert(string_splitn("Hello, world!", ",", 2)[2] == " world!", "string_splitn failed., got " .. string_splitn("Hello, world!", ",", 2)[2])

    for k, v in string_splitn("a=b=c!", "=", 1) do
        print(k, v)
    end
end

return string_splitn