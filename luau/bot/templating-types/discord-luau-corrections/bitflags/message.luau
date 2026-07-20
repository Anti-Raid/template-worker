--[[
	Implementation of the Discord Application Bitflags in Luau

	https://discord.com/developers/docs/resources/message#message-object-message-flags
]]

local bit = require("../vendor/bit")

local MessageBitflag = {}

MessageBitflag.Interface = {}
MessageBitflag.Prototype = {}

--[[
	this message has been published to subscribed channels (via Channel Following)
]]
function MessageBitflag.Prototype.crossposted(self: MessageBitflag)
	local targetBit = bit.lshift(1, 0)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
	this message originated from a message in another channel (via Channel Following)
]]
function MessageBitflag.Prototype.isCrossposted(self: MessageBitflag)
	local targetBit = bit.lshift(1, 1)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
	do not include any embeds when serializing this message
]]
function MessageBitflag.Prototype.suppressEmbeds(self: MessageBitflag)
	local targetBit = bit.lshift(1, 2)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
    set the suppress embeds bitflag
]]
function MessageBitflag.Prototype.setSuppressEmbeds(self: MessageBitflag)
    local targetBit = bit.lshift(1, 2)

    self.flag = bit.bor(self.flag, targetBit)
end

--[[
	the source message for this crosspost has been deleted (via Channel Following)
]]
function MessageBitflag.Prototype.sourceMessageDeleted(self: MessageBitflag)
	local targetBit = bit.lshift(1, 3)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
	this message came from the urgent message system
]]
function MessageBitflag.Prototype.urgent(self: MessageBitflag)
	local targetBit = bit.lshift(1, 4)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
	this message has an associated thread, with the same id as the message
]]
function MessageBitflag.Prototype.hasThread(self: MessageBitflag)
	local targetBit = bit.lshift(1, 5)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
	this message is only visible to the user who invoked the Interaction
]]
function MessageBitflag.Prototype.ephemeral(self: MessageBitflag)
	local targetBit = bit.lshift(1, 6)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
set the ephemeral bitflag
]]
function MessageBitflag.Prototype.setEphemeral(self: MessageBitflag)
    local targetBit = bit.lshift(1, 6)

    self.flag = bit.bor(self.flag, targetBit)
end

--[[
	this message is an Interaction Response and the bot is "thinking"
]]
function MessageBitflag.Prototype.loading(self: MessageBitflag)
	local targetBit = bit.lshift(1, 7)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
set the loading bitflag
]]
function MessageBitflag.Prototype.setLoading(self: MessageBitflag)
    local targetBit = bit.lshift(1, 7)

    self.flag = bit.bor(self.flag, targetBit)
end

--[[
	this message failed to mention some roles and add their members to the thread
]]
function MessageBitflag.Prototype.failedToMentionSomeRolesInThread(self: MessageBitflag)
	local targetBit = bit.lshift(1, 8)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
	this message will not trigger push and desktop notifications
]]
function MessageBitflag.Prototype.suppressNotifications(self: MessageBitflag)
	local targetBit = bit.lshift(1, 12)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
set the suppress notifications bitflag
]]
function MessageBitflag.Prototype.setSuppressNotifications(self: MessageBitflag)
    local targetBit = bit.lshift(1, 12)

    self.flag = bit.bor(self.flag, targetBit)
end

--[[
	this message is a voice message
]]
function MessageBitflag.Prototype.isVoiceMessage(self: MessageBitflag)
	local targetBit = bit.lshift(1, 13)

	return bit.band(self.flag, targetBit) == targetBit
end

--[[
set the voice message bitflag
]]
function MessageBitflag.Prototype.setVoiceMessage(self: MessageBitflag)
    local targetBit = bit.lshift(1, 13)

    self.flag = bit.bor(self.flag, targetBit)
end

function MessageBitflag.Interface.new(flag: number): MessageBitflag
	local self = setmetatable(
		{
			flag = flag,
		} :: MessageBitflag,
		{ __index = MessageBitflag.Prototype }
	) :: any

	return self
end

export type MessageBitflag = typeof(MessageBitflag.Prototype) & {
	flag: number,
}

return MessageBitflag.Interface