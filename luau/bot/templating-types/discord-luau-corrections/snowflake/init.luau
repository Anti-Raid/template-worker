--[[
	Wrapper around Discord snowflakes, allwowing developers to query information about a snowflake
]]

local datetime = require"@antiraid/datetime"

local DISCORD_EPOCH = 1420070400000

-- Snowflakes are 64-bit, so they are parsed with Luau's built-in integer library
-- (typesext no longer provides U64/bitu64). Every integer.* argument must be an integer.
local function parse(snowflake: string): integer
	return integer.fromstring(snowflake) or error(`Invalid snowflake "{snowflake}"`)
end

local function field(snowflake: string, mask: number?, shift: number): number
	local n = parse(snowflake)
	if mask then n = integer.band(n, integer.create(mask)) end
	if shift > 0 then n = integer.rshift(n, integer.create(shift)) end
	return integer.tonumber(n)
end

local Snowflake = {}

Snowflake.Interface = {}
Snowflake.Prototype = {}

--[[
	Returns the timestamp of the snowflake
]]
function Snowflake.Prototype.getTimestamp(self: Snowflake)
	local timestamp = field(self.snowflake, nil, 22) + DISCORD_EPOCH

	return datetime.UTC:fromTime((timestamp / 1000))
end

--[[
	Returns the internal worker ID of the snowflake
]]
function Snowflake.Prototype.getInternalWorkerId(self: Snowflake)
	return field(self.snowflake, 0x3E0000, 17)
end

--[[
	Returns the internal process ID of the snowflake
]]
function Snowflake.Prototype.getInternalProcessId(self: Snowflake)
	return field(self.snowflake, 0x1F000, 12)
end

--[[
	Returns the increment of the snowflake
]]
function Snowflake.Prototype.getIncrement(self: Snowflake)
	return field(self.snowflake, 0xFFF, 0)
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