--!strict
local commandBuilder = require "@discord-types/builders/interaction/interaction"
local apitypes = require "@discord-types/apiTypes"
local data = require"@antiraid-ext/frameworkv2/context"
local units = require"@antiraid-ext/frameworkv2/unit"
local ActionRowBuilder = require"@discord-types/builders/message/components/actionRow"
local ButtonBuilder = require"@discord-types/builders/message/components/button"
local typesext = require"@antiraid/typesext"
local mode = require"../../auxutils/lockdowns/mode"
local datetime = require"@antiraid/datetime"
local managers = require "../../auxutils/managers/managers"
-- Lockdown modes
local FullServerLockdown = require"../../auxutils/lockdowns/modes/fullserverlockdown"
local SingleChannelLockdown = require"../../auxutils/lockdowns/modes/singlechannellockdown"
local RoleLockdown = require"../../auxutils/lockdowns/modes/rolelockdown"

local command = commandBuilder.new({
    name = "lockdowns",
})
:addIntegrationType("GuildInstall")
:setType("ChatInput")
:addContext("Guild")
:setDescription("Manage lockdowns")
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("list")
        :setDescription("List all currently active lockdowns")  
        :build()      
    end
)
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("full")
        :setDescription("Launch a full server lockdown")  
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("reason")
                :setDescription("The reason for the lockdown")
                :setRequired(true)  
                :build()
            end
        )
        :build()      
    end
)
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("channel")
        :setDescription("Lock a single channel down") 
        :option(
            function(opt) 
                return opt
                :setType("Channel")
                :setName("channel")
                :setDescription("The channel to lock down")
                :setRequired(true)  
                :build()
            end
        ) 
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("reason")
                :setDescription("The reason for the lockdown")
                :setRequired(true)  
                :build()
            end
        )
        :build()      
    end
)
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("role")
        :setDescription("Lock a single role down") 
        :option(
            function(opt) 
                return opt
                :setType("Role")
                :setName("role")
                :setDescription("The role to lock down")
                :setRequired(true)  
                :build()
            end
        ) 
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("reason")
                :setDescription("The reason for the lockdown")
                :setRequired(true)  
                :build()
            end
        )
        :build()      
    end
)
:option(
    function(opt) 
        return opt
        :setType("SubCommand")
        :setName("remove")
        :setDescription("Remove a lockdown from this server")  
        :option(
            function(opt) 
                return opt
                :setType("String")
                :setName("id")
                :setDescription("The ID of the lockdown to remove")
                :setRequired(true)
                :setAutocompleteEnabled(true)  
                :build()
            end
        )
        :build()      
    end
)
:build()

local LOCKDOWN_EMBED_EXPIRY = datetime.timedelta_minutes(5)

local function createLockdown(
    data: data.CommandContext,
    lockdownType: mode.LockdownMode, 
    reason: string,
    ephemeral: boolean
): nil
    local lockdownset = managers.getmanagers(data.ctx).lockdownset
    
    local lockdowntestfailed = lockdownset.test(lockdownType)
    if lockdowntestfailed then 
        local autofixCustomId = `lockdowns:autofix:${typesext.randstring(16)}`
        data.framework.components.addwithexpiry(autofixCustomId, function(data: data.MessageComponentContext): nil 
            data.interaction.dropcomponent() -- Make sure to remove the callback so it doesn't get called again
            data.interaction.reply{
                embeds = {
                    {
                        title = "Starting lockdown autofixer",
                        description = "Please wait while the lockdown autofixer tries to fix the issues",
                    }
                },
                ephemeral = true
            }
            
            lockdownset.autofix(lockdownType)
            data.interaction.reply{
                embeds = {
                    {
                        title = "Lockdown autofixer finished",
                        description = "The lockdown autofixer has finished. You should be able to start the requested lockdown now",
                    },
                }
            }
            return nil
        end, LOCKDOWN_EMBED_EXPIRY)

        local autofixcomps = {
            ActionRowBuilder.new()
                :addComponent(
                    ButtonBuilder.new()
                    :setStyle("Grey")
                    :setLabel("Try Fixing Automatically")
                    :setCustomId(autofixCustomId)
                    :build()
                )
                :build() :: apitypes.ComponentObjects
        }

        data.interaction.replysimpleembed("Lockdown Failed", "We need to make some changes before you can lock down your server", units.RED_COLOR, autofixcomps)

        return nil
    end
        -- A lockdown may take a long time to apply, so we need to make sure the user knows that something is happening
        data.interaction.reply{
            embeds = {
                {
                    title = "Lockdown in progress",
                    description = "Please wait while the lockdown is applied",
                }
            }
        }

        local result = lockdownset.apply(lockdownType, reason)

        data.interaction.replyedit{
            embeds = {
                {
                    title = "Lockdown started",
                    description = "A lockdown has been started with an ID of " .. result.id .. "\n" .. (if reason then "Reason: " .. reason else "") .. ("\nType: " .. lockdownType.string_form()),
                },
            }
        }
        return nil
end

local function removeLockdown(
    data: data.CommandContext,
    id: string
): nil
    local lockdownset = managers.getmanagers(data.ctx).lockdownset

    -- A lockdown may take a long time to remove, so we need to make sure the user knows that something is happening
    data.interaction.reply{
        embeds = {
            {
                title = "Lockdown removal in progress",
                description = "Please wait while the lockdown is removed",
            }
        }
    }

    lockdownset.remove(id)

    data.interaction.replyedit{
        embeds = {
            {
                title = "Lockdown removed",
                description = "The requested lockdown has been removed",
                color = 0x00FF00,
            },
        }
    }

    return nil
end

local function run(data: data.CommandContext): nil
    local cmdname = data.command.nameList[2]
    local lockdownset = managers.getmanagers(data.ctx).lockdownset
    if cmdname == "list" then
        local lockdowns = lockdownset.manager.list()
        if #lockdowns == 0 then 
            return data.interaction.replysimpleembed("No active lockdowns", "There are no active lockdowns on this server", units.RED_COLOR)
        end

        local lockdownsList: {apitypes.EmbedFieldObject} = {}
        --setmetatable(lockdownsList, interop.array_metatable)
        for _, lockdown in lockdowns do 
            table.insert(lockdownsList, {
                name = lockdown.type.string_form(),
                value = "Reason: " .. (if lockdown.reason then lockdown.reason else "No reason specified") .. "\n" .. (if lockdown.created_at then "Created at: <t:" .. lockdown.created_at.timestamp_seconds .. ">" else "Created at: Unknown") .. "\nID: ``" .. lockdown.id .. "``",
                inline = true,
            } :: apitypes.EmbedFieldObject)
        end
        
        data.interaction.reply{
            embeds = {
                {
                    title = "Active Lockdowns",
                    description = "The following lockdowns are currently active on this server",
                    fields =  lockdownsList,
                    color = 0x00FF00,
                },
            }
        }
        return nil
    elseif cmdname == "full" then 
        data.interaction.assertpermission("lockdowns.full")
        local reason = data.args.string("reason")

        if not reason then 
            return data.interaction.replysimpleembed("No reason specified", "You must specify a reason for the server lockdown", units.RED_COLOR)
        end

        local lockdownType = FullServerLockdown()

        return createLockdown(
            data,
            lockdownType,
            reason,
            false
        )
    elseif cmdname == "channel" then
        data.interaction.assertpermission("lockdowns.scl")
        local reason = data.args.string("reason")
        local channel = data.args.channelid("channel")

        if not reason then 
            return data.interaction.replysimpleembed("No reason specified", "You must specify a reason for the channel lockdown", units.RED_COLOR)
        end
        if not channel then 
            return data.interaction.replysimpleembed("No channel specified", "You must specify a channel for the channel lockdown", units.RED_COLOR)
        end

        local lockdownType = SingleChannelLockdown(channel)

        return createLockdown(
            data,
            lockdownType,
            reason,
            false
        )
    elseif cmdname == "role" then
        data.interaction.assertpermission("lockdowns.role")
        local reason = data.args.string("reason")
        local role = data.args.roleid("role")

        if not reason then 
            return data.interaction.replysimpleembed("No reason specified", "You must specify a reason for the role lockdown", units.RED_COLOR)
        end
        if not role then 
            return data.interaction.replysimpleembed("No role specified", "You must specify a role for the role lockdown", units.RED_COLOR)
        end

        local lockdownType = RoleLockdown(role)

        return createLockdown(
            data,
            lockdownType,
            reason,
            false
        )
    elseif cmdname == "remove" then
        data.interaction.assertpermission("lockdowns.remove")
        local id = data.args.string("id")

        if not id then 
            return data.interaction.replysimpleembed("No ID specified", "You must lockdown ID to remove", units.RED_COLOR)
        end

        return removeLockdown(
            data,
            id
        )
    else 
        return data.interaction.replysimpleembed("Error processing command", "Unknown subcommand", units.RED_COLOR)
    end
end

local function autocompleter(data: data.CommandAutocompleteContext): {apitypes.ApplicationCommandOptionChoiceObject}
    local cmdname = data.command.nameList[2]

    if cmdname == "remove" then 
        local id = data.args.string("id")
        if not id then 
            return {}
        end

        local lockdownSet = managers.getmanagers(data.ctx).lockdownset
        local lockdowns = lockdownSet.manager.list()

        local autocompleteResults = {}  
        for _, lockdown in lockdowns do 
            if #autocompleteResults >= 25 then break end
            table.insert(autocompleteResults, {
                name = lockdown.type.string_form() .. " [" .. lockdown.id .. "]",
                value = lockdown.id,
            } :: apitypes.ApplicationCommandOptionChoiceObject)
        end

        return autocompleteResults
    else 
        return {}
    end
end

return {
    command = command,
    run = run,
    autocompleter = autocompleter,
}