export type null = typeof(setmetatable({} :: {
    read __phantom: number
}, {} :: {
    __eq: (self: any, other: any) -> boolean,
}))

local null: null = {} :: null

return {
    null = null,
    array_metatable = {
        __index = function(tab, key)
            return rawget(tab, key) -- Placeholder for AntiRaid's actual array metatable
        end
    },
    memusage = function()
        return 0
    end,
    memory_limit = function()
        return 0
    end,
}