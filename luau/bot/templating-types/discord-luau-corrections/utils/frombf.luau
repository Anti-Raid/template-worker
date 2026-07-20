local set = require"@antiraid-ext/set"

local function fromBitflag(bitflag: integer, flags: {[string]: integer}): {string} 
	local bf = bitflag
	local permissions = {}
	for flag, permission in flags do
		if integer.band(bf, permission) == (permission :: any) then
			table.insert(permissions, flag)
		end
	end

	return permissions
end

local function fromBitflagToSet(bitflag: integer, flags: {[string]: integer}): set.Set<string> 
	local bf = bitflag
	local permissions = set.Set() :: set.Set<string>
	for flag, permission in flags do
		if integer.band(bf, permission) == permission then
			permissions.add(flag)
		end
	end

	return permissions
end


return {
	fromBitflag = fromBitflag,
	fromBitflagToSet = fromBitflagToSet
}