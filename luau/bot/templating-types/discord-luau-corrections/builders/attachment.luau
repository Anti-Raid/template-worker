--!nocheck
-- -- Does not type under new type solver yet

--[[
	Implementation of discords Attachment object as a Luau builder.	

	https://discord.com/developers/docs/resources/message#attachment-object
]]--

local Primitives = require("@antiraid-core/primitives")

local Attachment = {}
 
Attachment.Prototype = {}
Attachment.Interface = {}

--[[
	Set the ID of the Attachment to use.
]]
function Attachment.Prototype.setId(self: Attachment, attachmentId: string?): Attachment
	self.attachmentId = attachmentId

	return self
end

--[[
    pub filename: String,
    pub description: Option<String>,
    pub content: Vec<u8>,
]]--

--[[
	Set the filename of the Attachment to use.
]]--
function Attachment.Prototype.setFilename(self: Attachment, filename: string): Attachment
	self.filename = filename

	return self
end

--[[
	Set the description of the Attachment to use.
]]
function Attachment.Prototype.setDescription(self: Attachment, description: string?): Attachment
	self.description = description

	return self
end

--[[
	Set the content of the Attachment to use.
]]--
function Attachment.Prototype.setContent(self: Attachment, content: {Primitives.u8}): Attachment
	self.content = content

	return self
end

--[[
	Responsible for building the Attachment JSON that can be parsed by the Discord API.
]]
function Attachment.Prototype.build(self: Attachment): JSON
	if(self.attachmentId) then
		return {
			id = self.attachmentId,
		}
	else
		return {
			filename = self.filename,
			description = self.description,
			content = self.content,
		}
	end
end

--[[
	Responsible for creating a new Attachment.

	```lua
	
	```
]]
function Attachment.Interface.new(resource: {
	attachmentId: string?,
	filename: string?,
	description: string?,
	content: {u8}?,
}?): Attachment
	local self = setmetatable({} :: Attachment, {
		__index = Attachment.Prototype,
	})

	if resource then
		if resource.attachmentId then
			self:setId(resource.attachmentId)
		end

		if resource.filename then
			self:setId(nil)
			self:setFilename(resource.filename)
		end

		if resource.description then
			self:setId(nil)
			self:setDescription(resource.description)
		end

		if resource.content then
			self:setId(nil)
			self:setContent(resource.content)
		end
	end

	return self
end

export type Attachment = typeof(Attachment.Prototype) & {
	attachmentId: string?,
	filename: string?,
	description: string?,
	content: {Primitives.u8}?,
}

export type JSON = typeof(Attachment.Prototype.build(nil :: any))

return Attachment.Interface
