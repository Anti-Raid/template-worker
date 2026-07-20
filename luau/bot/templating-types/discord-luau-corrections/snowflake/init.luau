--[[
	Wrapper around Discord snowflakes, allwowing developers to query information about a snowflake
]]

local u64 = require"@antiraid/typesext".U64
local bit = require"@antiraid/typesext".bitu64
local datetime = require"@antiraid/datetime"

local DISCORD_EPOCH = 1420070400000

local Snowflake = {}

Snowflake.Interface = {}
Snowflake.Prototype = {}

--[[
	Returns the timestamp of the snowflake
]]
function Snowflake.Prototype.getTimestamp(self: Snowflake)
	local snowflake = u64(self.snowflake)
	local timestamp = bit.rshift(snowflake, 22) + DISCORD_EPOCH

	return datetime.UTC:fromTime((timestamp / 1000))
end

--[[
	Returns the internal worker ID of the snowflake
]]
function Snowflake.Prototype.getInternalWorkerId(self: Snowflake)
	local snowflake = u64(self.snowflake)
	local internalWorkerId = bit.rshift(bit.band(snowflake, 0x3E0000), 17)

	return tonumber(tostring(internalWorkerId))
end

--[[
	Returns the internal process ID of the snowflake
]]
function Snowflake.Prototype.getInternalProcessId(self: Snowflake)
	local snowflake = u64(self.snowflake)
	local internalProcessId = bit.rshift(bit.band(snowflake, 0x1F0000), 12)

	return tonumber(tostring(internalProcessId))
end

--[[
	Returns the increment of the snowflake
]]
function Snowflake.Prototype.getIncrement(self: Snowflake)
	local snowflake = u64(self.snowflake)
	local internalProcessId = bit.band(snowflake, 0xFFF)

	return tonumber(tostring(internalProcessId))
end

--[[
	Creates a new snowflake
]]
function Snowflake.Interface.new(snowflake: string): Snowflake
	return setmetatable(
		{
			snowflake = snowflake,
		},
		{ __index = Snowflake.Prototype }
	) :: Snowflake
end

export type Snowflake = typeof(Snowflake.Prototype) & {
	snowflake: string,
}

return Snowflake.Interface