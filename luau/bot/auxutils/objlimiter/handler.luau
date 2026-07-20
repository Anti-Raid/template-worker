local RateStrategy = require "./RateStrategy"
local IntervalStrategy = require "./IntervalStrategy"
local types = require "./types"

local MAX_DEPTH = 10

--- Given an ObjLimitBlock, evaluates it and returns whether the object is limited (true) or not (false)
local function EvaluateObjLimitBlock(block: types.ObjLimitBlock): boolean
    if block.type == "Interval" then
        return IntervalStrategy(block.mode)
    elseif block.type == "Rate" then
        return RateStrategy(block.mode)
    else
        return false
    end
end

--- Evaluate a ObjLimit recursively. Returns true if the object is limited, else false.
---
--- Fails closed (returns true) if the depth exceeds MAX_DEPTH or if any other abnormal input is passed.
local function evaluateObjLimit(objLimit: types.ObjLimit, depth: number): boolean
    if depth > MAX_DEPTH then return true end
    if objLimit.type == "Single" then return EvaluateObjLimitBlock(objLimit.block)
    elseif objLimit.type == "Multiple" then 
        local left = evaluateObjLimit(objLimit.left, depth + 1)
        local right = evaluateObjLimit(objLimit.right, depth + 1)
        if objLimit.op == "AND" then return left and right
        elseif objLimit.op == "OR" then return left or right
        elseif objLimit.op == "XOR" then return left ~= right
        else return true end
    else return true end
end

--- Evaluate a ObjLimit recursively. Returns true if the object is limited, else false.
local function EvaluateObjLimit(objLimit: types.ObjLimit): boolean
    return evaluateObjLimit(objLimit, 0)
end

return {
    EvaluateObjLimitBlock = EvaluateObjLimitBlock,
    EvaluateObjLimit = EvaluateObjLimit
}