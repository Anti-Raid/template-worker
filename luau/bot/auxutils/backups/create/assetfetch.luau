local Primitives = require"@antiraid-core/primitives"
local apitypes = require"@discord-types/apiTypes"
local blob = require"@antiraid-core/blob"
local cdn = require"@antiraid-ext/system/net".Cdn

export type Asset = "icon" | "banner" | "splash"
local ASSETS = {
    "icon",
    "banner",
    "splash",
}

local function fetchAsset(ctx: Primitives.TemplateContext, g: apitypes.GuildObject, icon: Asset): blob.Blob? 
    local url = nil
    if icon == "icon" then
        if not g.icon then return nil end
        url = `https://cdn.discordapp.com/icons/{g.id}/{g.icon}.jpg?size=512`
    elseif icon == "banner" then
        if not g.banner then return nil end
        url = `https://cdn.discordapp.com/banners/{g.id}/{g.banner}.jpg?size=512`
    elseif icon == "splash" then
        if not g.splash then return nil end
        url = `https://cdn.discordapp.com/splashes/{g.id}/{g.splash}.jpg?size=512`
    else
        error(`Unknown asset type: {icon}`)
    end

    return cdn(ctx).downloadfromdiscordasblob(url)
end

local function fetchAssets(ctx: Primitives.TemplateContext, g: apitypes.GuildObject, assets: {Asset}): {[Asset]: blob.Blob?}
    local fetchedAssets = {}
    for _, asset in assets do
        fetchedAssets[asset] = fetchAsset(ctx, g, asset)
    end
    return fetchedAssets
end

local function fetchAllAssets(ctx: Primitives.TemplateContext, g: apitypes.GuildObject): {[Asset]: blob.Blob?}
    return fetchAssets(ctx, g, ASSETS)
end

return {
    fetchAsset = fetchAsset,
    fetchAssets = fetchAssets,
    fetchAllAssets = fetchAllAssets,
    ASSETS = ASSETS,
}