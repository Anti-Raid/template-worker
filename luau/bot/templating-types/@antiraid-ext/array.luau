--!nocheck
local interop = require("@antiraid/interop")

-- Create new array() function to allow easily creating arrays
-- 
-- This function sets the array_metatable as well
local function array(...)
    local arr = {}
    local packed = table.pack(...)
    for i = 1, select('#', ...) do
        table.insert(arr, packed[i])
    end

    setmetatable(arr, interop.array_metatable)
    return arr
end

local function internalProtectedEquals(o1, o2, ignore_mt, callList)
    if o1 == o2 then return true end
    local o1Type = type(o1)
    local o2Type = type(o2)
    if o1Type ~= o2Type then return false end
    if o1Type ~= 'table' then return false end

    -- add only when objects are tables, cache results
    local oComparisons = callList[o1]
    if not oComparisons then
        oComparisons = {}
        callList[o1] = oComparisons
    end
    -- false means that comparison is in progress
    oComparisons[o2] = false

    if not ignore_mt then
        local mt1 = getmetatable(o1)
        if mt1 and mt1.__eq then
            --compare using built in method
            return o1 == o2
        end
    end

    local keySet = {}
    for key1, value1 in pairs(o1) do
        local value2 = o2[key1]
        if value2 == nil then return false end

        local vComparisons = callList[value1]
        if not vComparisons or vComparisons[value2] == nil then
            if not internalProtectedEquals(value1, value2, ignore_mt, callList) then
                return false
            end
        end

        keySet[key1] = true
    end

    for key2, _ in pairs(o2) do
        if not keySet[key2] then
            return false
        end
    end

    -- comparison finished - objects are equal do not compare again
    oComparisons[o2] = true
    return true
end

--- Compares two objects for equality, including arrays and tables.
---
---@param o1 any|table First object to compare
---@param o2 any|table Second object to compare
---@param ignore_mt boolean True to ignore metatables (a recursive function to tests tables inside tables)
local function arraysEqual(o1, o2, ignore_mt)
    return internalProtectedEquals(o1, o2, ignore_mt or true, {})
end

--- Converts a value to an array. 
---
--- If the value is already an array, it is returned as is with a special ``array_metatable`` set as its metatable.
---
--- If the value is not an array, it is wrapped in an array.
local function toArray<T>(a: T): T 
    if typeof(a) == "table" then
        if getmetatable(a) == interop.array_metatable then
            return a
        end
        return setmetatable(a, interop.array_metatable) :: T
    else
        return array(a) :: T
    end
end

--- Slices a table from `from` to `to`.
---
--- If `from` is nil, it defaults to 1. If `to` is nil, it defaults to the length of the table.
--- Returns a new table containing the sliced elements.
---
--- Errors if ``to > #arr`` or ``from < 1``
---
---@param arr table The table to slice
---@param from number? The starting index (1-based)
---@param to number? The ending index (1-based)
---@return table A new table containing the sliced elements
local function slice<T>(arr: {T}, from: number?, to: number?): {T}
    local from_r = if from == nil then 1 else from
    local to_r = if to == nil then #arr else to

    if to_r > #arr or from_r < 1 then 
        error("slice out of bounds: to_r > #arr or from_r < 1 (" .. to_r .. ">" .. #arr .. " and " .. from_r .. "<1)")
    end

    local copy: {T} = table.create(to_r - from_r + 1)
    table.move(arr, from_r, to_r, 1, copy)
    return copy
end

return {
    array = array,
    arraysEqual = arraysEqual,
    toArray = toArray,
    slice = slice,
}