local blob = require"@antiraid-core/blob"

--- @param value: any
--- @param pretty: boolean? (Whether to pretty-print the JSON string. Defaults to false
--- @return string (The JSON string representation of the value.)
--- Creates a json string from a value 'v'
local function tojsonstring(value: any, pretty: boolean?): string
    error("Implemented internally in AntiRaid runtime!")
end

--- @param jsonstring: string (The JSON string or buffer containing the JSON data to parse.)
--- @return any (The Lua value represented by the JSON string.)
--- Parses a JSON string and returns the corresponding Lua value.
local function fromjsonstring(jsonstring: blob.BlobTaker): any
    error("Implemented internally in AntiRaid runtime!")
end

return {
    tojsonstring = tojsonstring,
    fromjsonstring = fromjsonstring
}