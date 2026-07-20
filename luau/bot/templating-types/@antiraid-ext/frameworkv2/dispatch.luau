--!strict
local dispatchutils = require"@antiraid-ext/events/dispatch"
local context = require"./context"
local LazyLoader = require"@antiraid-ext/sync/lazyload"
local InteractionCreate = require "@antiraid-ext/events/discord/InteractionCreate"
local WebSettings = require "@antiraid-ext/events/antiraid/WebSettings"
local Primitives = require"@antiraid-core/primitives"
local Interaction = require"@discord-types/interaction"
local appcommands = require"@antiraid-ext/appcommands"
local errorHandler = require"@antiraid-ext/utils/errorhandle"

type LdrType = {
    ctx: Primitives.TemplateContext,
    commands: {[string]: context.Command} | {context.Command},

}

local frameworkldr = LazyLoader<<context.Framework, LdrType>>(function(ldr) 
    local fw = context.Framework(ldr.ctx)
    for _, command in ldr.commands do 
        fw.commands.add(command.command.name, command)
    end
    return fw
end)

--- Sets up the event dispatch table to the framework.
---
--- This is the main entry point for the framework to handle events.
---
--- Currently, ``dispatch`` handles app commands and button callbacks
---
--- @param evt Primitives.Event The event to dispatch to
--- @param ctx Primitives.TemplateContext The context to use for the event.
--- @return nil
local function setup(commands: {[string]: context.Command} | {context.Command}, settingsHandler: context.SettingsHandler | nil, ...: dispatchutils.EventCall)
    return dispatchutils.eventList(
        InteractionCreate(function(ctx, interaction) 
            local framework = frameworkldr.get({ctx=ctx, commands=commands})
            if interaction.type == Interaction.InteractionType.ApplicationCommand then 
                local parsedcmd = appcommands.parseApplicationCommand(interaction)
                if not parsedcmd or not #parsedcmd.nameList then return end

                local command = framework.commands.get(parsedcmd.nameList[1])
                if not command then -- Likely another template's command
                    return 
                end
                local cctx = context.CommandContext(framework, interaction, parsedcmd)
                local ok, cerr = xpcall(command.run, errorHandler.errorHandler, cctx)
                if not ok then 
                    local err = (cerr :: any) :: errorHandler.HandledError
                    cctx.interaction.replyerror(err)
                end

            elseif interaction.type == Interaction.InteractionType.MessageComponent then 
                if not interaction.data or not interaction.data.custom_id then return end
                local cb = framework.components.get(interaction.data.custom_id)
                if not cb then return end
                local mctx = context.MessageComponentContext(framework, interaction)
                local ok, cerr = xpcall(cb, errorHandler.errorHandler, mctx)
                if not ok then 
                    local err = (cerr :: any) :: errorHandler.HandledError
                    mctx.interaction.replyerror(err)
                end

            elseif interaction.type == Interaction.InteractionType.ApplicationCommandAutocomplete then
                local parsedcmd = appcommands.parseApplicationCommand(interaction)
                if not parsedcmd or not #parsedcmd.nameList then return end

                local command = framework.commands.get(parsedcmd.nameList[1])
                if not command or not command.autocompleter then -- Likely another template's command
                    return 
                end
                local cctx = context.CommandAutocompleteContext(framework, interaction, parsedcmd)
                local ok, cerr = xpcall(command.autocompleter, errorHandler.errorHandler, cctx)
                if not ok then 
                    local err = (cerr :: any) :: errorHandler.HandledError
                    ctx.stream.publish("debug", { message = err, source = "<framework>" })
                    return
                end

                local results = cerr 
                if not cctx.interaction.responded() then 
                    cctx.interaction.autocomplete(results)
                end

            end
        end),
        WebSettings(function(ctx, event, author)
            if not settingsHandler then return nil end -- settings is unsupported
            local framework = frameworkldr.get({ctx=ctx, commands=commands})
            if event.type == "fetch_page" then 
                local sctx = context.SettingsFetchPageContext(framework, author)
                return settingsHandler.fetchpage(sctx)
            elseif event.type == "form_action" then
                assert(type(event.__tloop_template_id) == "string", "no __tloop_template_id sent in WebSettings event, this is a violation of the settings spec")
                local action = settingsHandler.formactions[event.formset_id]
                if not action then return nil end -- setting has no actions tied to it
                local sctx = context.SettingsFormActionContext(framework, author, event.action_button_id, event.formset_id, event.form_id, event.form_data)
                return action(sctx)
            end
            return nil
        end),
        ...
    )
end

return setup