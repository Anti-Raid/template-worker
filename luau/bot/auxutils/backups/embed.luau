local Primitives = require "@antiraid-core/primitives"
local apitypes = require "@discord-types/apiTypes"
local ActionRowBuilder = require"@discord-types/builders/message/components/actionRow"
local ButtonBuilder = require"@discord-types/builders/message/components/button"
local EmojiBuilder = require"@discord-types/builders/emoji"
local datetime = require"@antiraid/datetime"
local units = require"@antiraid-ext/frameworkv2/unit"
local UncachedKeyManager = require "@antiraid-ext/uncachedkeymanager"
local managers = require "../../auxutils/managers/managers"

export type EmbedData = {
    key: string,
    mdm: UncachedKeyManager.UncachedKeyManager<managers.BackupMetadata>,
    createdat: datetime.DateTime,
}

local function toembed(
    ctx: Primitives.TemplateContext,
    ed: EmbedData,
    showDownloadLink: boolean?
): (apitypes.EmbedObject, {apitypes.ComponentObjects})
    local components: {apitypes.ComponentObjects} = {}

    local description = string.format(
        "Created at: <t:%d:R>",
        ed.createdat.timestamp_seconds
    )

    if showDownloadLink and ed.key then 
        -- Create presigned URL
        local furl = ed.mdm.signbloburl(ed.key)
        description ..= string.format("\n\n[Download backup](%s)", furl.url)

        table.insert(
            components,
            ActionRowBuilder.new()
            :addComponent(
                ButtonBuilder.new()
                :setStyle("Blurple")
                :setLabel("Download")
                :setEmoji(
                    EmojiBuilder.new()
                    :setName("🔥")
                    :build()
                )
                :setUrl(furl.url)
                :build()
            )
            :build() :: apitypes.ComponentObjects
        )
    end

    local embed: apitypes.EmbedObject = {
        title = "Status",
        description = description,
        color = units.GREEN_COLOR,
        footer = {
            text = "Backup ID: " .. (ed.key or "Unset")
        },
    }

    return embed, components
end

return {
    toembed = toembed
}