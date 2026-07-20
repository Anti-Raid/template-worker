# Framework

AntiRaid provides a optional framework for easy command handling. A framework consists of a base (called a `FrameworkBase` internally) (such as `Simple` for normal use and `Flow` which was designed for AntiRaid's no-code Flow UI) as well as a set of basic/builtin structures for easy command handling which altogether make up a `Framework`.

## Command Handling

## Pagination API

Framework provides a simple pagination API for easy pagination of embeds. This is useful for commands that return a list of items, such as `urlblock list` and `backups list`.

As a simple example:

```luau
-- Send as paginated set of embeds
local urls = URLBlockManager(data.ctx).list()

if #urls == 0 then
    return interactionutils.replySimple(
        data.ctx,
        data.interaction,
        nil,
        {
            {
                title = "No URLs Blocked",
                description = "There are no URLs currently blocked in this server.",
                color = 0x00ff00, -- Green color
            },
        },
        true
    )
end

local function getUrlEmbed(idx: number): apitypes.EmbedObject
    if idx < 1 or idx > #urls then
        error("Index out of bounds: " .. idx .. " for URLs list of size " .. #urls)
    end
    local block = urls[idx]
    return {
        title = "Blocked URL #" .. idx,
        description = "URL: `" .. block.url .. "`\nReason: `" .. block.reason .. "`\nBlocked by: <@" .. block.user_id .. ">",
        color = 0xFF0000, -- Red color
        fields = {
            {
                name = "Created At",
                value = os.date("%Y-%m-%d %H:%M:%S", block.created_at.timestamp_seconds),
                inline = true,
            },
        },
    }
end

paginate.paginate(data, {
    id = "urlblock",
    embed = getUrlEmbed,
    numPages = #urls
})
return nil
```

Here, we first check if there are any URLs blocked. If there are none, we send a simple reply indicating that. If there are URLs, we define the pagination function `getUrlEmbed` that takes the current page as input (`idx`) and returns the embed for that URL block. Finally, we then call `paginate` with the necessary parameters to create the pagination.
