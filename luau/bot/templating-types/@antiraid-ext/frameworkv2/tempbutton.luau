--!strict
local typesext = require "@antiraid/typesext"
local ButtonBuilder = require"@discord-types/builders/message/components/button"
local ActionRowBuilder = require "@discord-types/builders/message/components/actionRow"
local apitypes = require "@discord-types/apiTypes"
local context = require"./context"
local datetime = require"@antiraid/datetime"

export type TempButtonOpts = {
    title: string,
    embed: () -> apitypes.EmbedObject,
    timeToExpiry: number? -- Optional, defaults to 120 seconds
}

export type TempButton = {
    button: apitypes.ButtonComponentObject,
    id: string,
    actionrow: apitypes.ComponentObjects,
    cancel: () -> ()
}

local TIME_TO_EXPIRY = datetime.timedelta_seconds(120)
local function tempbutton(ctx: context.CommandContext, opts: TempButtonOpts): TempButton
    local timeToExpiry = if opts.timeToExpiry then datetime.timedelta_seconds(opts.timeToExpiry ) else TIME_TO_EXPIRY -- Default to 120 seconds if not provided

    local tempbuttonid = `${typesext.randstring(16)}`

    local tempbutton = ButtonBuilder.new()
    :setStyle("Grey")
    :setLabel(opts.title)
    :setCustomId(tempbuttonid)
    :build()
    
    local userid = ctx.interaction.userid() -- Current user id who invoked pagination
    ctx.framework.components.addwithexpiry(tempbuttonid, function(ctx: context.MessageComponentContext) 
        local interactor = ctx.interaction.userid()
        if userid ~= interactor then return nil end
        
        local embed = opts.embed()
        if not embed then 
            return nil -- This should never happen
        end

        ctx.interaction.reply({
            embeds = {embed},
            components = {},
            ephemeral = true
        })

        return nil
    end, timeToExpiry)

    return {
        button = tempbutton,
        id = tempbuttonid,
        actionrow = ActionRowBuilder.new():addComponent(tempbutton):build() :: apitypes.ComponentObjects,
        cancel = function() 
            ctx.framework.components.remove(tempbuttonid)
        end
    }
end

return {
    tempbutton = tempbutton
}