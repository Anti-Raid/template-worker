--!strict
local discord = require"@discord-types/apiTypes"
local discordRestTypes = require "@discord-types/restTypes"
local Primitives = require"@antiraid-core/primitives"
local settings = require"@antiraid-core/settings"
local userinfomanager = require"@antiraid-ext/utils/userinfo"
local canModeratorDo = require "@antiraid-ext/utils/modhierarchy".canModeratorDo
local appcommands = require"@antiraid-ext/appcommands"
local channel = require"@antiraid/channel"
local datetime = require"@antiraid/datetime"
local discordP = require"@antiraid-core/plugins/discord"
local InteractionCallbackType = require "@discord-types/interaction".InteractionCallbackType
local InteractionType = require "@discord-types/interaction".InteractionType
local errorHandler = require"@antiraid-ext/utils/errorhandle"
local ActionRowBuilder = require"@discord-types/builders/message/components/actionRow"
local ButtonBuilder = require"@discord-types/builders/message/components/button"
local array_metatable = require"@antiraid/interop".array_metatable
local unit = require"./unit"
local net = require"@antiraid-ext/system/net"
local assertext = require"@antiraid-ext/assert"

export type Command = {
    --- Discord command definition.
    command: discordRestTypes.CreateGuildApplicationCommandRequest,
    --- The function that runs when the command is executed.
    run: (data: CommandContext) -> nil,
    --- The function that runs when the command is autocompleted.
    autocompleter: ((ctx: CommandAutocompleteContext) -> {discord.ApplicationCommandOptionChoiceObject})?,
}

--- A set of ID'd callbacks that can be attached (with optional expiry)
export type Callbacks<T> = {
    --- Adds to the callback list
    read add: (id: string, callback: T) -> (),
    --- Adds to the callback list with a expiry at which the callback is automatically removed
    read addwithexpiry: (id: string, callback: T, expirein: datetime.TimeDelta) -> (),
    --- Drops a callback.
    read remove: (id: string) -> (),
    --- Gets a callback from the framework.
    read get: (id: string) -> T?,
}

local function Callbacks<T>(): Callbacks<T> 
    local callbacks: {[string]: T} = {}

    local expiryhandles: {[string]: channel.KeyHandle<string>} = {}
    local delayqueue = channel.DelayChannel<<string>>()
    local expiryloopstarted = false    

    local function _expireifneeded()
        if not expiryloopstarted then
            expiryloopstarted = true
            task.spawn(function() 
                while true do 
                    local item = delayqueue:next()
                    expiryhandles[item] = nil
                    callbacks[item] = nil
                end
            end)
        end
    end


    return table.freeze{
        add = function(id: string, cb: T) 
            assert(not callbacks[id], "Callback with this ID has already been registered")
            callbacks[id] = cb 
        end,
        addwithexpiry = function(id: string, cb: T, expirein: datetime.TimeDelta)
            assert(not callbacks[id], "Callback with this ID has already been registered")
            assert(not expiryhandles[id], "Callback with this ID has already been registered")
            _expireifneeded() -- Spin up expiry loop to clean out expired callbacks
            callbacks[id] = cb
            expiryhandles[id] = delayqueue:add(id, expirein)
        end,
        remove = function(id: string) 
            callbacks[id] = nil 
            if expiryhandles[id] then 
                expiryhandles[id]:cancel() 
                expiryhandles[id] = nil
            end
        end,
        get = function(id: string) return callbacks[id] end
    }
end

--- The base framework context shared across all framework invokations
export type Framework = {
    --- The underlying template context
    read ctx: Primitives.TemplateContext,
    --- Underlying cdn downloader
    read cdn: net.Cdn,
    --- Underlying statistics (+ other metadata) handler
    read meta: net.Meta,
    --- The underlying user info manager for managing user permissions
    read userinfomanager: userinfomanager.UserInfoManager,
    --- Message component callbacks
    read components: Callbacks<(MessageComponentContext) -> nil>,
    --- Command callbacks
    read commands: Callbacks<Command>,
}

--- Creates a base framework
local function Framework(ctx: Primitives.TemplateContext): Framework
    local userinfomanager = userinfomanager(ctx)
    local componentcbs = Callbacks<<(MessageComponentContext) -> nil>>()
    local commandcbs = Callbacks<<Command>>()

    return table.freeze{
        ctx = ctx,
        cdn = net.Cdn(ctx),
        meta = net.Meta(ctx),
        userinfomanager = userinfomanager,
        components = componentcbs,
        commands = commandcbs
    }
end

export type InteractionReplyData = {
    content: string?,
    embeds: {discord.EmbedObject}?,       -- Adjust these to match your actual discord types
    components: {discord.ComponentObjects}?,
    allowed_mentions: discord.AllowedMentionObject?,
    flags: number?,
    tts: boolean?,
    ephemeral: boolean?
}

--- Holds a interaction and provides methods for simple manipulation of a interaction
--- (without needing to manually manage stuff like hasEdited etc.)
export type InteractionManager = {
    --- The underlying template context
    read ctx: Primitives.TemplateContext,
    --- The underlying interaction
    read interaction: discord.InteractionObject,
    --- Edits the original interaction response
    read editoriginalresponse: (data: discordRestTypes.EditWebhookMessageRequest) -> discordP.LazyMessageObject,
    --- Creates a new interaction response
    ---
    --- In most cases, you probably want `reply` which will either create or followup for you
    read createoriginalresponse: (data: discordRestTypes.CreateInteractionRequest) -> nil,
    --- Acks the original interaction response (for interactions that take a bit more time to make initial response with)
    read ack: () -> nil,
    --- If the interaction is a message component, update the message the component was attached to
    read updatemessage: (data: discord.InteractionCallbackDataEx) -> nil,
    --- If the interaction is a message component, defer updating of the message the component was attached to (for interactions that take a bit more time to make initial response with)
    read deferupdatemessage: () -> nil,
    --- Sends a followup message after the interaction has already been responded to
    read createfollowup: (data: discordRestTypes.CreateFollowupMessageRequest) -> discordP.LazyMessageObject,
    --- Replies to an interaction. Either responds directly or creates a followup message as desired
    read reply: (data: InteractionReplyData) -> discordP.LazyMessageObject?,
    --- Replies to an interaction. Either responds directly or creates a followup message as desired
    read replyedit: (data: InteractionReplyData) -> discordP.LazyMessageObject?,
    --- Replies to an interaction with an error
    read replyerror: (err: errorHandler.HandledError) -> discordP.LazyMessageObject?,
    --- Helper method to create a simple embed reply
    read replysimpleembed: (title: string, description: string, color: number, components: {discord.ComponentObjects}?) -> nil,
    --- Replies to a autocomplete with a set of choices
    read autocomplete: (choices: {discord.ApplicationCommandOptionChoiceObject}) -> nil,
    --- Returns the custom id in the interaction if any
    read customid: () -> string?,
    --- Returns if the user has responded to the interaction ever.
    read responded: () -> boolean,
    --- Returns the underlying user id
    read userid: () -> string,
    --- Returns the underlying user
    read user: () -> discord.UserObject,
    --- Returns the userinfo of the interacted-with user
    read userinfo: () -> userinfomanager.UserInfo,
    --- Assert that the user `u` has permission `p` erroring if not
    read assertpermission: (perm: string) -> userinfomanager.UserInfo,
    --- If a component, drop it from callbacks
    read dropcomponent: () -> ()
}

--- Creates a new InteractionManager, if `inithasresponded` is true, then the InteractionManager will consider the interaction
--- as already being responded to
local function InteractionManager(ctx: Primitives.TemplateContext, framework: Framework, interaction: discord.InteractionObject, inithasresponded: boolean?): InteractionManager 
    local hasresponded = inithasresponded or false

    local function editoriginalresponse(data: discordRestTypes.EditWebhookMessageRequest)
        assert(hasresponded, "You must first reply to a interaction before editing it")
        return ctx.discord:edit_original_interaction_response({
            interaction_token = interaction.token,
            data = data,
        })
    end

    local function createoriginalresponse(data: discordRestTypes.CreateInteractionRequest)
        assert(not hasresponded, "You cannot create multiple original responses")
        ctx.discord:create_interaction_response({
            interaction_id = interaction.id,
            interaction_token = interaction.token,
            data = data,
        })
        hasresponded = true
        return nil
    end

    local function ack() 
        return createoriginalresponse({
            type = InteractionCallbackType.DeferredChannelMessageWithSource,
        } :: discordRestTypes.CreateInteractionRequest)
    end

    local function updatemessage(data: discord.InteractionCallbackData)
        assert(interaction.type == InteractionType.MessageComponent, "interaction must be a message component")
        -- Note: This requires the interaction type to be a Message Component (Type 3)
        return createoriginalresponse({
            type = InteractionCallbackType.UpdateMessage,
            data = data,
        } :: discordRestTypes.CreateInteractionRequest)
    end

    local function deferupdatemessage()
        assert(interaction.type == InteractionType.MessageComponent, "interaction must be a message component")
        -- Note: This requires the interaction type to be a Message Component (Type 3)
        return createoriginalresponse({
            type = InteractionCallbackType.DeferredUpdateMessage,
        } :: discordRestTypes.CreateInteractionRequest)
    end

    local function createfollowup(data: discordRestTypes.CreateFollowupMessageRequest)
        assert(hasresponded, "You must first respond to the interaction before sending a followup")
        -- Note: Ensure this matches the exact method name in your Discord wrapper 
        -- (e.g., create_followup_message or execute_webhook)
        return ctx.discord:create_followup_message({
            interaction_token = interaction.token,
            data = data,
        })
    end

    local function reply(data: InteractionReplyData): discordP.LazyMessageObject?
        local finalFlags = data.flags or 0
        if data.ephemeral then
            finalFlags = bit32.bor(finalFlags, bit32.lshift(1, 6)) -- Ephemeral
        end
        if finalFlags ~= 0 then data.flags = finalFlags end
        
        -- fix for empty embeds and components etc
        if data.embeds and #data.embeds == 0 then data.embeds = setmetatable({}, array_metatable) :: any end
        if data.components and #data.components == 0 then data.components = setmetatable({}, array_metatable) :: any end
        
        if not hasresponded then
            return createoriginalresponse({
                type = InteractionCallbackType.ChannelMessageWithSource,
                data = data,
            } :: discordRestTypes.CreateInteractionRequest)
        else
            return createfollowup(data :: discordRestTypes.ExecuteWebhookRequest)
        end
    end

    local function replyedit(data: InteractionReplyData): discordP.LazyMessageObject?
        local finalFlags = data.flags or 0
        if data.ephemeral then
            finalFlags = bit32.bor(finalFlags, bit32.lshift(1, 6)) -- Ephemeral
        end
        if finalFlags ~= 0 then data.flags = finalFlags end
        
        if not hasresponded then
            return createoriginalresponse({
                type = InteractionCallbackType.ChannelMessageWithSource,
                data = data,
            } :: discordRestTypes.CreateInteractionRequest)
        else
            return editoriginalresponse(data :: discordRestTypes.EditWebhookMessageRequest)
        end
    end

    local function replysimpleembed(title: string, description: string, color: number, components: {discord.ComponentObjects}?): nil
        reply({
            embeds = {
                {
                    title = title,
                    description = description,
                    color = color
                }
            },
            components = components
        })
        return nil
    end

    local function replyerror(err: errorHandler.HandledError) 
        local comps = {
            ActionRowBuilder.new()
            :addComponent(
                ButtonBuilder.new()
                :setStyle("Blurple")
                :setLabel("Support Server")
                :setUrl(ctx.btd().website)
                :build()
            )
            :build()
        }
        
        local res = reply({
            embeds = {err.embed},
            components = comps :: any,
            ephemeral = true
        })

        return res
    end

    local function autocomplete(choices: {discord.ApplicationCommandOptionChoiceObject})
        if #choices > 25 then
            -- Results are limited to 25 so we need to trim the table
            local trimmedResults = {}
            for i = 1, 25 do
                trimmedResults[i] = choices[i]
            end
            choices = trimmedResults
        end
        
        return createoriginalresponse({
            type = InteractionCallbackType.ApplicationCommandAutocompleteResult,
            data = {
                choices = if choices then setmetatable(choices, array_metatable) else nil,
            },
        } :: discordRestTypes.CreateInteractionRequest)
    end

    local function customid() return interaction.data and interaction.data.custom_id end
    local function responded() return hasresponded end

    local function userid(): string
        return if interaction.user then interaction.user.id elseif (interaction.member and interaction.member.user) then interaction.member.user.id else error("No user found")
    end
    local function user(): discord.UserObject 
        return if interaction.user then interaction.user elseif (interaction.member and interaction.member.user) then interaction.member.user else error("No user found")
    end
    local function userinfo(): userinfomanager.UserInfo 
        return framework.userinfomanager.get(userid(), interaction)
    end
    local function assertpermission(cap: string) 
        local userinfo = userinfo()
        canModeratorDo(userinfo, cap)
        return userinfo
    end
    local function dropcomponent() 
        local customid = customid()
        if customid then framework.components.remove(customid) end
    end

    return table.freeze{
        ctx = ctx,
        interaction = interaction,
        editoriginalresponse = editoriginalresponse,
        createoriginalresponse = createoriginalresponse,
        ack = ack,
        updatemessage = updatemessage,
        deferupdatemessage = deferupdatemessage,
        createfollowup = createfollowup,
        reply = reply,
        replyedit = replyedit,
        replyerror = replyerror,
        replysimpleembed = replysimpleembed,
        customid = customid,
        autocomplete = autocomplete,
        responded = responded,
        userid = userid,
        user = user,
        userinfo = userinfo,
        assertpermission = assertpermission,
        dropcomponent = dropcomponent
    }
end

export type CommandArgs = {
    -- string
    read string: (name: string) -> string?,
    -- integer
    read integer: (name: string) -> number?,
    -- boolean
    read boolean: (name: string) -> boolean?,
    -- user
    read user: (name: string) -> discord.UserObject?,
    read member: (name: string) -> discord.GuildMemberObject?,
    read userid: (name: string) -> string?,
    -- channel
    read channel: (name: string) -> discord.ChannelObject?,
    read channelid: (name: string) -> string?,
    -- role
    read role: (name: string) -> discord.GuildRoleObject?,
    read roleid: (name: string) -> string?,
    -- number
    read number: (name: string) -> number?,
    -- attachment
    read attachment: (name: string) -> discord.AttachmentObject?,
    -- get option
    read get: (name: string) -> appcommands.ResolvedOption?,
    --- time in seconds (nname -> name of option with number of `unit`, unitname -> unit (seconds/minutes/hours/days))
    read time: (nname: string, unitname: string) -> number?,
}

local function CommandArgs(cmd: appcommands.ParsedApplicationCommand): CommandArgs
    local mapped: {[string]: appcommands.ResolvedOption}? = nil

    local function get(name: string): appcommands.ResolvedOption?
        if mapped == nil then 
            -- Create map of all opts lazily
            local map = {}
            for _, opt in cmd.options do
                map[opt.name] = opt
            end
            mapped = map
        end
        assert(mapped, "mapped is now not nil here")
        return mapped[name]
    end

    return table.freeze{
        string = function(name: string): string? 
            local opt = get(name)
            if opt and opt.value.kind == "String" then 
                return opt.value.value
            end
            return nil
        end,
        integer = function(name: string): number? 
            local opt = get(name)
            if opt and opt.value.kind == "Integer" then 
                return opt.value.value
            end
            return nil
        end,
        boolean = function(name: string): boolean? 
            local opt = get(name)
            if opt and opt.value.kind == "Boolean" then 
                return opt.value.value
            end
            return nil
        end,
        user = function(name: string): discord.UserObject? 
            local opt = get(name)
            if opt and opt.value.kind == "User" then 
                return opt.value.user
            end
            return nil
        end,
        member = function(name: string): discord.GuildMemberObject? 
            local opt = get(name)
            if opt and opt.value.kind == "User" then 
                return opt.value.member
            end
            return nil
        end,
        userid = function(name: string): string? 
            local opt = get(name)
            if opt then
                if opt.value.kind == "User" then return opt.value.user.id 
                elseif opt.value.kind == "Unresolved" and opt.value.type == "User" then return opt.value.id 
                end
            end
            return nil
        end,
        channel = function(name: string): discord.ChannelObject? 
            local opt = get(name)
            if opt and opt.value.kind == "Channel" then 
                return opt.value.channel
            end
            return nil
        end,
        channelid = function(name: string): string? 
            local opt = get(name)
            if opt then 
                if opt.value.kind == "Channel" then return opt.value.channel.id 
                elseif opt.value.kind == "Unresolved" and opt.value.type == "Channel" then return opt.value.id 
                end
            end
            return nil
        end,
        role = function(name: string): discord.GuildRoleObject? 
            local opt = get(name)
            if opt and opt.value.kind == "Role" then 
                return opt.value.role
            end
            return nil
        end,
        roleid = function(name: string): string? 
            local opt = get(name)
            if opt then 
                if opt.value.kind == "Role" then return opt.value.role.id 
                elseif opt.value.kind == "Unresolved" and opt.value.type == "Role" then return opt.value.id 
                end
            end
            return nil
        end,
        number = function(name: string): number? 
            local opt = get(name)
            if opt then 
                if opt.value.kind == "Number" then return opt.value.value
                elseif opt.value.kind == "Integer" then return opt.value.value
                end
            end
            return nil
        end,
        attachment = function(name: string): discord.AttachmentObject? 
            local opt = get(name)
            if opt and opt.value.kind == "Attachment" then 
                return opt.value.attachment
            end
            return nil
        end,
        attachmentid = function(name: string): string? 
            local opt = get(name)
            if opt then 
                if opt.value.kind == "Attachment" then return opt.value.attachment.id 
                elseif opt.value.kind == "Unresolved" and opt.value.type == "Attachment" then return opt.value.id 
                end
            end
            return nil
        end,
        get = get,
        time = function(nname: string, unitname: string): number? 
            local nopt = get(nname)
            local nvalue = nil
            if nopt then 
                if nopt.value.kind == "Number" then nvalue = nopt.value.value
                elseif nopt.value.kind == "Integer" then nvalue = nopt.value.value
                end
            end
            if not nvalue then return nil end

            local unitopt = get(unitname)
            local unitvalue = if unitopt and unitopt.value.kind == "String" then unitopt.value.value else nil
            if not unitvalue then return nil end

            return unit.toseconds(nvalue, unitvalue)
        end
    }
end

export type BaseContext = {
    --- The base framework
    read framework: Framework,
    --- The underlying context
    read ctx: Primitives.TemplateContext,
}

--- Context for a message component interaction
export type MessageComponentContext = BaseContext & {
    --- The interaction
    read interaction: InteractionManager,
}

local function MessageComponentContext(framework: Framework, interaction: discord.InteractionObject): MessageComponentContext
    assert(interaction.type == InteractionType.MessageComponent, "interaction must be a message component")
    return table.freeze{
        framework = framework,
        ctx = framework.ctx,
        interaction = InteractionManager(framework.ctx, framework, interaction)
    }
end

--- Context for a command invokation
export type CommandContext = BaseContext & {
    --- The interaction
    read interaction: InteractionManager,
    --- The parsed command data
    read command: appcommands.ParsedApplicationCommand,
    --- The command arguments
    read args: CommandArgs,
}

local function CommandContext(framework: Framework, interaction: discord.InteractionObject, command: appcommands.ParsedApplicationCommand): CommandContext
    assert(interaction.type == InteractionType.ApplicationCommand, "interaction must be a command")
    return table.freeze{
        framework = framework,
        ctx = framework.ctx,
        interaction = InteractionManager(framework.ctx, framework, interaction),
        command = command,
        args = CommandArgs(command)
    }
end

--- Context for a command invokation
export type CommandAutocompleteContext = BaseContext & {
    --- The interaction
    read interaction: InteractionManager,
    --- The parsed command data
    read command: appcommands.ParsedApplicationCommand,
    --- The command arguments
    read args: CommandArgs,
}

local function CommandAutocompleteContext(framework: Framework, interaction: discord.InteractionObject, command: appcommands.ParsedApplicationCommand): CommandAutocompleteContext
    assert(interaction.type == InteractionType.ApplicationCommand, "interaction must be a message component")
    return table.freeze{
        framework = framework,
        ctx = framework.ctx,
        interaction = InteractionManager(framework.ctx, framework, interaction),
        command = command,
        args = CommandArgs(command)
    }
end

--- The function that runs when a setting is invoked by the client.
export type SettingsHandler = {
    --- Fetch the settings page returning a settings.Page
    fetchpage: (data: SettingsFetchPageContext) -> settings.Page,
    --- A map of formset id's to a function that performs form actions for said formset
    formactions: {[string]: (data: SettingsFormActionContext) -> ()}
}

--- Context for a settings invokation
export type SettingsFetchPageContext = BaseContext & {
    --- The user who is performing the settings invokation
    read author: string
}

local function SettingsFetchPageContext(framework: Framework, author: string): SettingsFetchPageContext
    return table.freeze{
        framework = framework,
        ctx = framework.ctx,
        author = author
    }
end

--- Context for a settings invokation
export type SettingsFormActionContext = BaseContext & {
    --- The user who is performing the settings invokation
    read author: string,
    --- The clicked action button's id
    read action_button_id: string, 
    --- ID of the form set containing the form
    read formset_id: string,
    --- The id of the form containing the action button
    read form_id: string,
    --- Where key is the element id, value is the inputted data
    read form_data: {[string]: any}?,

    --- Get string from args
    read argstring: (name: string) -> string,
    --- Get number from args
    read argnumber: (name: string) -> number,
    --- Get boolean from args
    read argboolean: (name: string) -> boolean,
    --- Get a list of strings from args
    read argstringlist: (name: string) -> {string}
}

local function SettingsFormActionContext(framework: Framework, author: string, action_button_id: string, formset_id: string, form_id: string, form_data: {[string]: any}?): SettingsFormActionContext
    return table.freeze{
        framework = framework,
        ctx = framework.ctx,
        author = author,
        action_button_id = action_button_id,
        formset_id = formset_id,
        form_id = form_id,
        form_data = form_data,
        argstring = function(name: string): string
            assert(form_data, "Validation Error: No form data sent!")
            local arg = form_data[name]
            return assertext.assertString(name, arg)
        end,
        argnumber = function(name: string): number
            assert(form_data, "Validation Error: No form data sent!")
            local arg = form_data[name]
            return assertext.assertFloat(name, arg)
        end,
        argboolean = function(name: string): boolean
            assert(form_data, "Validation Error: No form data sent!")
            local arg = form_data[name]
            return assertext.assertBoolean(name, arg)
        end,
        argstringlist = function(name: string): {string}
            assert(form_data, "Validation Error: No form data sent!")
            local arg = form_data[name]
            return assertext.assertStringArray(name, arg)
        end
    }
end

return {
    Callbacks = Callbacks,
    Framework = Framework,
    InteractionManager = InteractionManager,
    MessageComponentContext = MessageComponentContext,
    CommandContext = CommandContext,
    CommandAutocompleteContext = CommandAutocompleteContext,
    SettingsFetchPageContext = SettingsFetchPageContext,
    SettingsFormActionContext = SettingsFormActionContext
}