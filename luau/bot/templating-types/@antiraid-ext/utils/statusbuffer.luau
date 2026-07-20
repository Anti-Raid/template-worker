--- Helper method to format a value
local function formatValue(value: any): string
    if type(value) == "table" then
        local s = "{"

        for k, v in value do
            if type(k) == "number" then
                s ..= string.format("%s, ", formatValue(v))
                continue
            end
            s ..= string.format("%s = %s, ", tostring(k), formatValue(v))
        end

        -- Remove the last comma and space
        if #s > 1 then
            s = string.sub(s, 1, -3)
        end

        s ..= "}"
        return s
    elseif type(value) == "string" then
        return value
    else
        return tostring(value)
    end
end

--- A status entry
export type Status = {
    ts: number,
    message: string
}

--- An StatusBuffer stores status and state info on a long-running job
export type StatusBuffer = {
    addStatus: (msg: string) -> (),
    getStatus: () -> {Status},
}

local function StatusBuffer(): StatusBuffer 
    local statuses: {Status} = {}
    local function addStatus(msg: string) 
        table.insert(statuses, {message = msg, ts = os.time()})
    end
    local function getStatus() return statuses end
    return {
        addStatus = addStatus,
        getStatus = getStatus
    }
end

type StringifyBufferTsMode = "Discord" | "BracketStart"

--- Utility method to convert a status buffer into a list of strings
---
--- If mode is set to "discord", ts is formatted as <t:{ts}:R> format
local function stringifybuffer(sb: StatusBuffer, tsmode: StringifyBufferTsMode?, maxlen: number?, permsgthreshold: number?): {string}
    local len = 0
    local statuses: {string} = {}
    
    for _, status in sb.getStatus() do 
        local add = status.message

        if permsgthreshold then
            if #add > permsgthreshold then
                add = string.sub(add, 1, permsgthreshold) .. "..."
            end
        end

        if tsmode == "Discord" then 
            add ..= string.format(" <t:%d:R>", status.ts)
        elseif tsmode == "BracketStart" then 
            add = `[{status.ts}] ` .. add 
        end

        len += (#add + 1)
        table.insert(statuses, add)

        if maxlen then 
            while len > maxlen and #statuses > 0 do 
                local removed = table.remove(statuses, 1)
                if removed then
                    len -= (#removed + 1)
                end
            end
        end
    end
    
    return statuses 
end

return { 
    StatusBuffer = StatusBuffer,
    stringifybuffer = stringifybuffer
}