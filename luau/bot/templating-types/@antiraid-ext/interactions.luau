local ApplicationCommandOptionType_Discord = require "@discord-types/interaction".ApplicationCommandOptionType
local applicationTypes = require("@discord-types/application")

--- Converts a string representation of an application command option type to its corresponding numeric value.
--- @param type The string representation of the application command option type.
--- @return The numeric value corresponding to the application command option type.
local function ApplicationCommandOptionType(
    type: "SubCommand" | "SubCommandGroup" | "String" | "Integer" | "Boolean" | "User" | "Channel" | "Role" | "Mentionable" | "Number" | "Attachment"
): number
    return ApplicationCommandOptionType_Discord[type] :: number
end

local function IntegrationType(type: "GuildInstall" | "UserInstall"): number
    return applicationTypes.IntegrationTypesConfig[type] :: number
end

return {
    ApplicationCommandOptionType = ApplicationCommandOptionType,
    IntegrationType = IntegrationType
}