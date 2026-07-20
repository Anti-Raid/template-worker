--!nocheck
-- Does not check under new type solver yet

local apiTypes = require("../../../apiTypes")
local prelude = require("../../../prelude")
local metadata = require("./metadata")

local Action = {}

Action.Prototype = {}
Action.Interface = {}

--[[
	Sets the type of the action.
]]
function Action.Prototype.setType(self: Action, type: apiTypes.AutomoderationActionType): Action
	self.type = type

	return self
end

--[[
	Sets the action metadata.
]]
function Action.Prototype.setMetadata(self: Action, metadata: metadata.JSON): Action
	self.metadata = metadata

	return self
end

--[[
	Builds the action object.
]]
function Action.Prototype.build(self: Action): JSON
	return {
		type = prelude.guild.ActionTypes[self.type],
		metadata = self.metadata,
	}
end

--[[
	Responsible for creating a new Action.

	```lua
	
	```
]]
function Action.Interface.new(resource: {
	type: apiTypes.AutomoderationActionType?,
	metadata: metadata.JSON?,
}?): Action
	local self = setmetatable({} :: Action, {
		__index = Action.Prototype,
	})

	if resource then
		if resource then
			if resource.type then
				self:setType(resource.type)
			end

			if resource.metadata then
				self:setMetadata(resource.metadata)
			end
		end
	end

	return self
end

export type Action = typeof(Action.Prototype) & {
	type: apiTypes.AutomoderationActionType,
	metadata: metadata.JSON?,
}

export type JSON = typeof(Action.Prototype.build(nil :: any))

return Action.Interface