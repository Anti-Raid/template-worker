-- assertX functions

--- Asserts that the value is a string and returns it.
local function assertString(what: string, value: any): string
    assert(type(value) == "string", "Expected a string, got " .. type(value) .. " for " .. what)
    return value
end

--- Asserts that the value is a string and one of the choices.
local function assertStringChoices(what: string, value: any, choices: {string}): string
    assert(type(value) == "string", "Expected a string, got " .. type(value) .. " for " .. what)
    for _, choice in choices :: {string} do
        if value == choice then
            return value
        end
    end
    error("Expected one of " .. table.concat(choices, ", ") .. ", got " .. value .. " for " .. what)
end

--- Asserts that the value is an integer and returns it.
local function assertInteger(what: string, value: any): number
    assert(type(value) == "number" and value % 1 == 0, "Expected an integer, got " .. type(value) .. " for " .. what)
    return value
end

--- Asserts that the value is a float and returns it.
local function assertFloat(what: string, value: any): number
    assert(type(value) == "number", "Expected a float, got " .. type(value) .. " for " .. what)
    return value
end

--- Asserts that the value is a boolean and returns it.
local function assertBoolean(what: string, value: any): boolean
    assert(type(value) == "boolean", "Expected a boolean, got " .. type(value) .. " for " .. what)
    return value
end

--- Asserts that the value is an array of strings and returns it.
local function assertStringArray(what: string, value: any): {string}
    assert(type(value) == "table", "Expected a array of strings, got " .. type(value))
    for _, v in value :: {string} do
        assert(type(v) == "string", "Expected a string in the array, got " .. type(v))
    end
    return value
end

--- Asserts that the value is an array of integers and returns it.
local function assertIntegerArray(what: string, value: any): {number}
    assert(type(value) == "table", "Expected a array of integers, got " .. type(value))
    for _, v in value :: {number} do
        assert(type(v) == "number" and v % 1 == 0, "Expected an integer in the array, got " .. type(v))
    end
    return value
end

--- Asserts that the value is an array of floats and returns it.
local function assertFloatArray(what: string, value: any): {number}
    assert(type(value) == "table", "Expected a array of floats, got " .. type(value))
    for _, v in value :: {number} do
        assert(type(v) == "number", "Expected a float in the array, got " .. type(v))
    end
    return value
end

--- Asserts that the value is an array of booleans and returns it.
local function assertBooleanArray(what: string, value: any): {boolean}
    assert(type(value) == "table", "Expected a array of booleans, got " .. type(value))
    for _, v in value :: {boolean} do
        assert(type(v) == "boolean", "Expected a boolean in the array, got " .. type(v))
    end
    return value
end

return {
    -- assertX functions
    assertString = assertString,
    assertStringChoices = assertStringChoices,
    assertInteger = assertInteger,
    assertFloat = assertFloat,
    assertBoolean = assertBoolean,
    assertStringArray = assertStringArray,
    assertIntegerArray = assertIntegerArray,
    assertFloatArray = assertFloatArray,
    assertBooleanArray = assertBooleanArray,
}