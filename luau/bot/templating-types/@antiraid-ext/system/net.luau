--!strict
local datetime = require("@antiraid/datetime")
local runtime = require("@antiraid-core/plugins/runtime")
local Primitives = require"@antiraid-core/primitives"

--- Helper function to execute and unwrap cdn syscall
local function cdncall(ctx: Primitives.TemplateContext, req: runtime.CdnCall): runtime.CdnResult
    local result = ctx.syscall({
        op = "Cdn",
        req = req
    })

    if result.op ~= "Cdn" then
        error(`expected Cdn response`, 3)
    end

    return result.res
end

--- Helper function to execute and unwrap meta syscall
local function metacall(ctx: Primitives.TemplateContext, req: runtime.MetaCall): runtime.MetaResult
    local result = ctx.syscall({
        op = "Meta",
        req = req
    })

    if result.op ~= "Meta" then
        error(`expected meta response`, 3)
    end

    return result.res
end

export type Cdn = {    
    read downloadfromdiscord: (url: string) -> buffer,
}

local function Cdn(ctx: Primitives.TemplateContext): Cdn 
    local function downloadfromdiscord(url: string): buffer
        local res = cdncall(ctx, {
            op = "DownloadFile",
            url = url
        })

        if res.op ~= "Buffer" then
            error(`[Cdn] downloadfromdiscord failed: unexpected response '{res.op}'`, 2)
        end

        return res.data
    end

    return table.freeze{
        downloadfromdiscord = downloadfromdiscord,
    }
end

export type Meta = {    
    read stats: () -> {
        total_guilds: number, total_users: number, last_started_at: datetime.DateTime
    },
}

local function Meta(ctx: Primitives.TemplateContext): Meta 
    local function stats()
        local res = metacall(ctx, {
            op = "GetStats",
        })

        if res.op ~= "Stats" then
            error(`[Cdn] stats failed: unexpected response '{res.op}'`, 2)
        end

        return res
    end


    return table.freeze{
        stats = stats
    }
end

return { Cdn = Cdn, Meta = Meta }