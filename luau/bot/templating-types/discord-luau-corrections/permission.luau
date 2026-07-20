local reflect = require("@discord-types/utils/table/reflect")

local bit = integer
local fromBf = require("@discord-types/utils/frombf")
local set = require("@antiraid-ext/set")

local function intshift(a: number, b: number) 
	return integer.lshift(integer.fromstring(tostring(a)), integer.fromstring(tostring(b)))
end

local PermissionSet: {[string]: integer} = {
	CreateInstantInvite = intshift(1, 0),
	KickMembers = intshift(1, 1),
	BanMembers = intshift(1, 2),
	Administrator = intshift(1, 3),
	ManageChannels = intshift(1, 4),
	ManageGuild = intshift(1, 5),
	AddReactions = intshift(1, 6),
	ViewAuditLog = intshift(1, 7),
	PrioritySpeaker = intshift(1, 8),
	Stream = intshift(1, 9),
	ViewChannel = intshift(1, 10),
	SendMessages = intshift(1, 11),
	SendTTSMessages = intshift(1, 12),
	ManageMessages = intshift(1, 13),
	EmbedLinks = intshift(1, 14),
	AttachFiles = intshift(1, 15),
	ReadMessageHistory = intshift(1, 16),
	MentionEveryone = intshift(1, 17),
	UseExternalEmojis = intshift(1, 18),
	ViewGuildInsights = intshift(1, 19),
	Connect = intshift(1, 20),
	Speak = intshift(1, 21),
	MuteMembers = intshift(1, 22),
	DeafenMembers = intshift(1, 23),
	MoveMembers = intshift(1, 24),
	UseVAD = intshift(1, 25),
	ChangeNickname = intshift(1, 26),
	ManageNicknames = intshift(1, 27),
	ManageRoles = intshift(1, 28),
	ManageWebhooks = intshift(1, 29),
	ManageGuildExpressions = intshift(1, 30),
	UseApplicationCommands = intshift(1, 31),
	RequestToSpeak = intshift(1, 32),
	ManageEvents = intshift(1, 33),
	ManageThreads = intshift(1, 34),
	CreatePublicThreads = intshift(1, 35),
	CreatePrivateThreads = intshift(1, 36),
	UseExternalStickers = intshift(1, 37),
	SendMessagesInThreads = intshift(1, 38),
	UseEmbeddedActivities = intshift(1, 39),
	ModerateMembers = intshift(1, 40),
	ViewCreatorMonetizationAnalytics = intshift(1, 41),
	UseSoundboard = intshift(1, 42),
	UseExternalSounds = intshift(1, 45),
	SendVoiceMessages = intshift(1, 46),
	SendPolls = intshift(1, 49),
	UseExternalApps = intshift(1, 50),
}

local Permissions = table.freeze(reflect(PermissionSet :: {[string]: integer}))

local PermissionsAll = 0i
for _, perm in PermissionSet do
	PermissionsAll = integer.bor(PermissionsAll, perm)
end

export type Permissions =
	"CreateInstantInvite"
	| "KickMembers"
	| "BanMembers"
	| "Administrator"
	| "ManageChannels"
	| "ManageGuild"
	| "AddReactions"
	| "ViewAuditLog"
	| "PrioritySpeaker"
	| "Stream"
	| "ViewChannel"
	| "SendMessages"
	| "SendTTSMessages"
	| "ManageMessages"
	| "EmbedLinks"
	| "AttachFiles"
	| "ReadMessageHistory"
	| "MentionEveryone"
	| "UseExternalEmojis"
	| "ViewGuildInsights"
	| "Connect"
	| "Speak"
	| "MuteMembers"
	| "DeafenMembers"
	| "MoveMembers"
	| "UseVad"
	| "ChangeNickname"
	| "ManageNicknames"
	| "ManageRoles"
	| "ManageWebhooks"
	| "ManageGuildExpressions"
	| "UseApplicationCommands"
	| "RequestToSpeak"
	| "ManageEvents"
	| "ManageThreads"
	| "CreatePublicThreads"
	| "CreatePrivateThreads"
	| "UseExternalStickers"
	| "SendMessagesInThreads"
	| "UseEmbeddedActivities"
	| "ModerateMembers"
	| "ViewCreatorMonetizationAnalytics"
	| "UseSoundboard"
	| "CreateGuildExpressions"
	| "CreateEvents"
	| "UseExternalSounds"
	| "SendVoiceMessages"
	| "SendPolls"
	| "UseExternalApps"

--- Given a bitflag of permissions, returns a table of permission strings
local function fromBitflag(bitflag: integer): {Permissions} 
	return fromBf.fromBitflag(bitflag, Permissions) :: any
end

--- Given a bitflag of permissions, returns a set of permission strings
local function fromBitflagToSet(bitflag: integer): set.Set<Permissions>
	return fromBf.fromBitflagToSet(bitflag, Permissions) :: any
end

--- Converts a table of permission strings to a bitflag
local function toBitflag(permissions: {Permissions}): integer
	local bitflag = 0i
	for _, permission in permissions do
		if not Permissions[permission] then
			error("Invalid permission: " .. permission)
		end
		bitflag = bit.bor(bitflag, Permissions[permission])
	end
	return bitflag
end

--- Converts a set of permission strings to a bitflag
local function toBitflagFromSet(permissions: set.Set<Permissions>): integer
	local bitflag = 0i
	for permission in permissions do
		if not Permissions[permission] then
			error("Invalid permission: " .. permission)
		end
		bitflag = bit.bor(bitflag, Permissions[permission])
	end
	return bitflag
end

--- Adds the given permissions to the bitflag
---
--- Unlike the other methods here, this method is non-destructive and does not modify the original bitflag.
--- (e.g. if Discord has new permissions we don't know about, those will still be preserved)
local function addToPerms(bf: integer, permissions: {Permissions}): integer
	local bitflag = toBitflag(permissions)
	return bit.bor(bf, bitflag)
end

--- Removes the given permissions from the bitflag
---
--- Unlike the other methods here, this method is non-destructive and does not modify the original bitflag.
--- (e.g. if Discord has new permissions we don't know about, those will still be preserved)
local function removeFromPerms(bf: integer, permissions: {Permissions}): integer
	local bitflag = toBitflag(permissions)
	return bit.band(bf, bit.bnot(bitflag))
end

local OverwriteObjectTypeRole = 0
local OverwriteObjectTypeMember = 1

return {
	Permissions = Permissions,
	PermissionSet = PermissionSet,
	PermissionsAll = PermissionsAll,
	fromBitFlag = fromBitflag,
	fromBitFlagToSet = fromBitflagToSet,
	toBitFlag = toBitflag,
	toBitflagFromSet = toBitflagFromSet,
	OverwriteObjectTypeRole = OverwriteObjectTypeRole,
	OverwriteObjectTypeMember = OverwriteObjectTypeMember,
	addToPerms = addToPerms,
	removeFromPerms = removeFromPerms,
}