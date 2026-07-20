local discord = require("@discord-types/apiTypes")
local InteractionType = require "@discord-types/interaction".InteractionType
local ApplicationCommandOptionType = require "@discord-types/interaction".ApplicationCommandOptionType

--- Parsed application command type created using `parseApplicationCommand`
export type ParsedApplicationCommand = {
    --- The base name of the slash command
    baseName: string,
    --- The command name list of the slash command
    nameList: {string},
    --- The inner data of the interaction
    data: discord.InteractionDataObject,
    --- The inner options of the slash command
    options: {ResolvedOption}
}

--- An option that is resolved from the slash command interaction options.
---
--- Note that Subcommand and SubcommandGroup will probably not be observed unless resolveOption is called directly outside
--- of parseApplicationCommand due to the extra parsing logic in `parseApplicationCommand` that unwraps subcommands and subcommand groups.
export type ResolvedOption = {
    name: string,
    value: ResolvedValue,
    focused: boolean,
    isMentionable: boolean,
}

export type ResolvedValue = {
    kind: "Subcommand",
    options: {ResolvedOption}
} | {
    kind: "SubcommandGroup",
    options: {ResolvedOption}
} |  {
    kind: "String",
    value: string
} | {
    kind: "Integer",
    value: number
} | {
    kind: "Boolean",
    value: boolean
} | {
    kind: "User",
    user: discord.UserObject,
    member: discord.GuildMemberObject?
} | {
    kind: "Channel",
    channel: discord.ChannelObject
} | {
    kind: "Role",
    role: discord.GuildRoleObject
} | {
    kind: "Number",
    value: number
} | {
    kind: "Attachment",
    attachment: discord.AttachmentObject
} | {
    kind: "Unresolved",
    type: "User" | "Channel" | "Role" | "Mentionable" | "Attachment",
    id: string
}

--- Resolves discords option into a more type-safe structure for handling slash commands
local function resolveOptions(opts: {discord.ApplicationCommandInteractionDataOptionObject}, resolved: discord.ResolvedDataStructure): {ResolvedOption} 
    local options = {}
    for _, opt in opts do 
        local value: ResolvedValue? = nil
        local isMentionable = false

        --[[
local ApplicationCommandOptionType = table.freeze(reflect({
	SubCommand = 1,
	SubCommandGroup = 2,
	String = 3,
	Integer = 4,
	Boolean = 5, 
	User = 6, 
	Channel = 7, 
	Role = 8, 
	Mentionable = 9, 
	Number = 10, 
	Attachment = 11, -- DONE
}))
        ]]

        if opt.type == ApplicationCommandOptionType.SubCommand then 
            local ops = resolveOptions(opt.options, resolved)
            value = { kind = "Subcommand" :: "Subcommand", options = ops }
        elseif opt.type == ApplicationCommandOptionType.SubCommandGroup then
            local ops = resolveOptions(opt.options, resolved)
            value = { kind = "SubcommandGroup" :: "SubcommandGroup", options = ops }
        elseif opt.type == ApplicationCommandOptionType.String then
            value = { kind = "String" :: "String", value = tostring(opt.value or "") }
        elseif opt.type == ApplicationCommandOptionType.Integer then
            value = { kind = "Integer" :: "Integer", value = tonumber(opt.value) or 0 }
        elseif opt.type == ApplicationCommandOptionType.Boolean then
            local boolvalue = false
            if type(opt.value) == "boolean" then 
                boolvalue = opt.value 
            end
            value = { kind = "Boolean" :: "Boolean", value = boolvalue }
        elseif opt.type == ApplicationCommandOptionType.User then
            local resolvedUser = nil
            local resolvedMember: discord.GuildMemberObject? = nil

            if resolved.users and type(opt.value) == "string" then
                resolvedUser = resolved.users[opt.value]
            end

            if resolvedUser and resolved.members and type(opt.value) == "string" then 
                resolvedMember = resolved.members[opt.value]
                if resolvedMember then resolvedMember.user = resolvedUser end
            end

            if resolvedUser then
                value = { kind = "User" :: "User", user = resolvedUser, member = resolvedMember }
            else
                value = { kind = "Unresolved" :: "Unresolved", id = tostring(opt.value or ""), type = "User" :: "User" } :: ResolvedValue
            end
        elseif opt.type == ApplicationCommandOptionType.Channel then
            local resolvedChannel = nil
            if resolved.channels and type(opt.value) == "string" then 
                resolvedChannel = resolved.channels[opt.value]
            end

            if resolvedChannel then
                value = { kind = "Channel" :: "Channel", channel = resolvedChannel }
            else
                value = { kind = "Unresolved" :: "Unresolved", id = tostring(opt.value or ""), type = "Channel" :: "Channel" } :: ResolvedValue
            end
        elseif opt.type == ApplicationCommandOptionType.Role then
            local resolvedRole = nil
            if resolved.roles and type(opt.value) == "string" then 
                resolvedRole = resolved.roles[opt.value]
            end

            if resolvedRole then
                value = { kind = "Role" :: "Role", role = resolvedRole }
            else
                value = { kind = "Unresolved" :: "Unresolved", id = tostring(opt.value or ""), type = "Role" :: "Role" } :: ResolvedValue
            end
        elseif opt.type == ApplicationCommandOptionType.Mentionable then
            isMentionable = true

            -- #1: User
            local resolvedUser = nil
            local resolvedMember: discord.GuildMemberObject? = nil

            if resolved.users and type(opt.value) == "string" then
                resolvedUser = resolved.users[opt.value]
            end

            if resolvedUser and resolved.members and type(opt.value) == "string" then 
                resolvedMember = resolved.members[opt.value]
                if resolvedMember then resolvedMember.user = resolvedUser end
            end

            if resolvedUser then
                value = { kind = "User" :: "User", user = resolvedUser, member = resolvedMember }
            end

            -- #2: Channel
            if not value then
                local resolvedChannel = nil
                if resolved.channels and type(opt.value) == "string" then 
                    resolvedChannel = resolved.channels[opt.value]
                end

                if resolvedChannel then
                    value = { kind = "Channel" :: "Channel", channel = resolvedChannel }
                end
            end

            -- #3: Role
            if not value then 
                local resolvedRole = nil
                if resolved.roles and type(opt.value) == "string" then 
                    resolvedRole = resolved.roles[opt.value]
                end
    
                if resolvedRole then
                    value = { kind = "Role" :: "Role", role = resolvedRole }
                end
            end

            -- #4: Out of options :(
            if not value then 
                value = { kind = "Unresolved" :: "Unresolved", id = tostring(opt.value or ""), type = "Mentionable" :: "Mentionable" } :: ResolvedValue
            end
        elseif opt.type == ApplicationCommandOptionType.Number then
            value = { kind = "Number" :: "Number", value = tonumber(opt.value) or 0 }
        elseif opt.type == ApplicationCommandOptionType.Attachment then
            local resolvedAttachment = nil
            if resolved.attachments and type(opt.value) == "string" then 
                resolvedAttachment = resolved.attachments[opt.value]
            end

            if resolvedAttachment then
                value = { kind = "Attachment" :: "Attachment", attachment = resolvedAttachment }
            else
                value = { kind = "Unresolved" :: "Unresolved", id = tostring(opt.value or ""), type = "Attachment" :: "Attachment" } :: ResolvedValue
            end
        end

        if value then
            options[#options + 1] = {
                name = opt.name,
                value = value,
                focused = opt.focused,
                isMentionable = isMentionable
            }
        end
    end

    return options
end

--- Parses a/an application/slash command from an AntiRaid interaction create event into a parsed slash command object.
local function parseApplicationCommand(interaction: discord.InteractionObject): ParsedApplicationCommand?
    if interaction.type ~= InteractionType.ApplicationCommand and interaction.type ~= InteractionType.ApplicationCommandAutocomplete then
        return nil
    end

    if not interaction.data then
        return nil -- No data, we can't really proceed from here
    end

    local data = interaction.data :: discord.InteractionDataObject

    local baseName = data.name

    -- Get options for further parsing
    local options = resolveOptions(data.options or {}, data.resolved or {} :: discord.ResolvedDataStructure)

    local namelist = {baseName} -- This will hold the final name list for the command

    --- Unwraps the command options to get the final list of names for subcommands and subcommand groups.
    local function unwrapCommandOptions(namelist: {string}, options: {ResolvedOption}): {ResolvedOption}
        if #options == 1 then
            local singleOption = options[1]
            if singleOption.value.kind == "Subcommand" then 
                namelist[#namelist + 1] = singleOption.name -- Append the subcommand name to the namelist
                -- Unwrap the subcommand options
                return unwrapCommandOptions(namelist, singleOption.value.options)
            elseif singleOption.value.kind == "SubcommandGroup" then
                namelist[#namelist + 1] = singleOption.name -- Append the subcommand group name to the namelist
                -- Unwrap the subcommand group options
                return unwrapCommandOptions(namelist, singleOption.value.options)
            end
        end

        return options
    end

    local unwrappedOptions = unwrapCommandOptions(namelist, options)

    return {
        baseName = baseName,
        nameList = namelist, -- The final name list after unwrapping
        data = interaction.data,
        options = unwrappedOptions, -- The resolved options for the command
    } :: ParsedApplicationCommand
end


--- ParsedCommandDataArgument is the type of a parsed command data argument
export type ParsedCommandDataArgument = {
    name: string,
    description: string,
    type: "String" | "Integer" | "Boolean" | "User" | "Channel" | "Role" | "Number" | "Attachment",
    required: boolean,
    choices: {discord.ApplicationCommandOptionChoiceObject}?,
    min_length: number?,
    max_length: number?,
}

--- ParsedCommandData is the parsed ``discordRestTypes.CreateGuildApplicationCommandRequest``
export type ParsedCommandData = {
    name: string,
    description: string?,
    options: {ParsedCommandDataArgument},
    subcommands: {ParsedCommandData},
}

export type AppCommandDataLike = {
    name: string,
    description: string?,
    options: {discord.ApplicationCommandOptionObject}?,
}

--- Parses a command data object from a discordRestTypes.CreateGuildApplicationCommandRequest into a parsed command data object.
local function parseApplicationCommandData(data: AppCommandDataLike): ParsedCommandData
    local parsedData: ParsedCommandData = {
        name = data.name,
        description = data.description,
        options = {},
        subcommands = {},
    }

    if not data.options then
        return parsedData
    end

    for _, option in data.options do
        if option.type == ApplicationCommandOptionType.SubCommand or option.type == ApplicationCommandOptionType.SubCommandGroup then
            local subcommandData = parseApplicationCommandData(option)
            table.insert(parsedData.subcommands, subcommandData)
            continue
        end

        local parsedOption = {
            name = option.name,
            description = option.description,
            type = if option.type == ApplicationCommandOptionType.String then "String"
                elseif option.type == ApplicationCommandOptionType.Integer then "Integer"
                elseif option.type == ApplicationCommandOptionType.Boolean then "Boolean"
                elseif option.type == ApplicationCommandOptionType.User then "User"
                elseif option.type == ApplicationCommandOptionType.Channel then "Channel"
                elseif option.type == ApplicationCommandOptionType.Role then "Role"
                elseif option.type == ApplicationCommandOptionType.Number then "Number"
                elseif option.type == ApplicationCommandOptionType.Attachment then "Attachment"
                else tostring(ApplicationCommandOptionType[option.type] or "Unknown: " .. tostring(option.type)),
            required = option.required or false,
            choices = option.choices,
            min_length = option.min_length,
            max_length = option.max_length,
        }

        table.insert(parsedData.options, parsedOption)
    end

    return parsedData
end

return {
    resolveOptions = resolveOptions,
    parseApplicationCommand = parseApplicationCommand,
    parseApplicationCommandData = parseApplicationCommandData,
}