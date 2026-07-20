--!strict
local typesext = require "@antiraid/typesext"
local ButtonBuilder = require"@discord-types/builders/message/components/button"
local ActionRowBuilder = require "@discord-types/builders/message/components/actionRow"
local apitypes = require "@discord-types/apiTypes"
local context = require"./context"
local datetime = require"@antiraid/datetime"

export type PaginationOpts = {
    id: string,
    embed: (currentIdx: number) -> apitypes.EmbedObject,
    numPages: number,
    timeToExpiry: number? -- Optional, defaults to 120 seconds
}

local TIME_TO_EXPIRY = datetime.timedelta_seconds(120)
local function paginate(ctx: context.CommandContext, opts: PaginationOpts): ()
    local currentIdx = 1
    local timeToExpiry = if opts.timeToExpiry then datetime.timedelta_seconds(opts.timeToExpiry ) else TIME_TO_EXPIRY -- Default to 120 seconds if not provided

    local jlPrevId = `{opts.id}:${typesext.randstring(16)}`
    local jlNextId = `{opts.id}:${typesext.randstring(16)}`

    local prevButton = ButtonBuilder.new()
    :setStyle("Grey")
    :setLabel("Previous Page")
    :setCustomId(jlPrevId)

    local nextButton = ButtonBuilder.new()
    :setStyle("Grey")
    :setLabel("Next Page")
    :setCustomId(jlNextId)
    
    local userid = ctx.interaction.userid() -- Current user id who invoked pagination
    ctx.framework.components.addwithexpiry(jlPrevId, function(ctx: context.MessageComponentContext) 
        local interactor = ctx.interaction.userid()
        if userid ~= interactor then return nil end
        
        currentIdx -= 1
        if currentIdx < 1 then
            currentIdx = 1
        end

        local embed = opts.embed(
            currentIdx
        )
        if not embed then 
            return nil -- This should never happen
        end

        if embed.title then
            embed.title ..= string.format(" (%d/%d)", currentIdx, opts.numPages)
        end

        ctx.interaction.updatemessage({
            embeds = {embed},
            components = {
                ActionRowBuilder.new()
                :addComponent(
                    prevButton:build()
                )
                :addComponent(
                    nextButton:build()
                )
                :build() :: apitypes.ComponentObjects
            }
        })

        return nil
    end, timeToExpiry)

    ctx.framework.components.addwithexpiry(jlNextId, function(ctx: context.MessageComponentContext): nil 
        local interactor = ctx.interaction.userid()
        if userid ~= interactor then return nil end
        currentIdx += 1
        if currentIdx > opts.numPages then
            currentIdx = opts.numPages
        end

        local embed = opts.embed(
            currentIdx
        )

        if embed.title then
            embed.title ..= string.format(" (%d/%d)", currentIdx, opts.numPages)
        end

        ctx.interaction.updatemessage({
            embeds = {embed},
            components = {
                ActionRowBuilder.new()
                :addComponent(
                    prevButton:build()
                )
                :addComponent(
                    nextButton:build()
                )
                :build() :: apitypes.ComponentObjects
            }
        })

        return nil    
    end, timeToExpiry)

    local embed = opts.embed(
        currentIdx
    )

    if embed.title then
        embed.title ..= string.format(" (%d/%d)", currentIdx, opts.numPages)
    end

    ctx.interaction.reply({
        embeds = {embed},
        components = {
            ActionRowBuilder.new()
            :addComponent(
                prevButton:build()
            )
            :addComponent(
                nextButton:build()
            )
            :build() :: apitypes.ComponentObjects
        }
    })
end

return {
    paginate = paginate
}