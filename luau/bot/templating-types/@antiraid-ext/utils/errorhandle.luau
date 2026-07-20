local apitypes = require"@discord-types/apiTypes"

--- The data of a custom error.
export type CustomError = {
    type: "MissingPermissions",
    perm: string,
    altPerm: string?,
} | string

export type HandledError = {
    --- The discord embed to send.
    embed: apitypes.EmbedObject,
    --- The traceback to rethrow, if any. If nil, do not rethrow.
    traceback: string?
}

--- A function that takes in a custom error and returns a handled error.
local function errorHandler(err: CustomError): HandledError
    if type(err) == "string" then
	local tb = debug.traceback(err, 2)
        tb = tostring(tb) :: string
        --error(tb .. "\n\n\n")
	if #tb > 2000 then 
            -- Limit to 1997 chars and add a ... at the end
            tb = tb:sub(1, 1997) .. "..."
        end

        return {
            embed = {
                title = "Something happened!",
                description = "Something happened while trying to process your request. If the problem persists, **please report it to our Support Server**\n```lua\n" .. tb .. "\n```",
                color = 0xFF0000,
            },
            traceback = tb,
        }
    elseif type(err) == "table" and err.type == "MissingPermissions" then
        if err.altPerm then
            return {
                embed = {
                    title = "Missing Permissions",
                    description = "You are missing the required permission: **" .. err.perm .. "** or the alternative Discord permission **" .. err.altPerm .. "**",
                    fields = {
                        {
                            name = "How to fix this?",
                            value = `Ask the server owner (or any administrator) to give you the **{err.perm}** permission on AntiRaid. Alternatively, ensure you have the **{err.altPerm}** permission in the server.`,
                            inline = false
                        },
                    },
                    color = 0xFF0000,
                },
            }
        end
        return {
            embed = {
                title = "Missing Permissions",
                description = "You are missing the required permission: **" .. err.perm .. "**",
                fields = {
                    {
                        name = "How to fix this?",
                        value = `Ask the server owner (or any administrator) to give you the **{err.perm}** permission on AntiRaid.`,
                        inline = false
                    },
                },
                color = 0xFF0000,
            },
        }
    else
        return {
            embed = {
                title = "Unknown Error",
                description = `An unknown error occurred. {err}`,
                color = 0xFF0000,
            },
            traceback = tostring(err),
        }
    end
end

return {
    errorHandler = errorHandler,
}
