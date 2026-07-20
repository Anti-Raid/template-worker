--!strict

--- Builtins internal template dispatch code for AntiRaid (heavy work in progress)
---
--- Note that this is INTERNAL and specific to how AntiRaid dispatches out to templates
---
--- Do not expect this to work outside of init

local setup = require "@antiraid-core/setup"
local Primitives = require"@antiraid-core/primitives"
local runtime = require"@antiraid-core/plugins/runtime"

local function main(syscall: runtime.RawSyscall, ts: runtime.TenantState, btd: runtime.BaseTenantData) 
    local data = setup.newinitloop(syscall, ts, btd) -- Get or create the TemplateLoopManager instance *once* on init
    
    local function dispatch(evt: Primitives.Event) 
        -- Special case for $UpdateTenantState
        if evt.name == "$UpdateTenantState" then 
            data.updatetenantstate(evt.data)
            return {} 
        end
        if type(evt.data) == "table" and type(evt.data.__tloop_template_id) == "string" then 
            return data.ctx.loop.dispatchSingle(evt, evt.data.__tloop_template_id)
        end
        return data.ctx.loop.dispatch(evt)
    end
    return dispatch
end

return main