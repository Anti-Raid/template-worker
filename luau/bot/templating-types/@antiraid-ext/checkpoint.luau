--!strict
local Primitives = require"@antiraid-core/primitives"
local KeyExpiryManager = require "@antiraid-ext/keyexpirymanager"
local sb = require "@antiraid-ext/utils/statusbuffer"
local datetime = require"@antiraid/datetime"
local typesext = require"@antiraid/typesext"

export type CheckpointContext<Data> = {
    --- The ID of the checkpoint, used to identify the checkpoint when resuming
    read id: string,
    --- The template context
    read tctx: Primitives.TemplateContext,
    --- The status buffer for the checkpointer
    read sb: sb.StatusBuffer,
    --- Returns if we are resuming from a checkpoint
    read resumed: boolean,
    --- slate stores temporary data for the checkpoint step
    read slate: {[string]: any},
    --- The intermediate data for the checkpoint. This will be passed to each step function and can be used to store any data needed to resume the task.
    read data: () -> Data,
    --- Save the intermediate state of the checkpoint. This will be used to resume from in the event of an interruption/crash/etc.
    read save: (data: Data) -> (),
}

export type CheckpointStep<Data> = {
    step: string, -- Unique identifier for the step. Used to determine where to resume from.
    fn: (ctx: CheckpointContext<Data>) -> Data?,
}

export type CheckpointManager<Data> = {
    --- Creates a new checkpoint
    new: (initialdata: Data, sb: sb.StatusBuffer?) -> ()
}

type DataWithStep<Data> = {
    step: string,
    data: Data,
}

--- Manages checkpoints for long-running tasks that may need to be resumed in the event of a VM crash etc.
---
--- @param ctx The template context
--- @param baseid A unique identifier for the checkpoint manager to allow different checkpoint managers to coexist/operate independently
--- @param expiry The amount of time after which checkpoints should expire and be automatically deleted. This is to prevent clutter and potential security issues from old checkpoints lingering around indefinitely.
--- @param steps The steps of the checkpoint, in order. Each step should have a unique step identifier. When resuming from a checkpoint, the step identifier will be used to determine which step to resume from.
local function CheckpointManager<Data>(ctx: Primitives.TemplateContext, baseid: string, expiry: datetime.TimeDelta, steps: {CheckpointStep<Data>}): CheckpointManager<Data>
    if #steps == 0 then error("Checkpoint must have at least one step") end
    local function loadinner(id: string, idatawithstep: DataWithStep<Data>, resumed: boolean, kexm: KeyExpiryManager.KeyExpiryManager<DataWithStep<Data>>, sb: sb.StatusBuffer) 
        local atstep = false
        
        -- May be mutated in place
        local datawithstep = idatawithstep

        local cctx: CheckpointContext<Data>
        cctx = table.freeze{
            tctx = ctx,
            sb = sb,
            data = function() return datawithstep.data end,
            id = id,
            resumed = resumed,
            slate = {},
            save = function(newdata: Data) 
                datawithstep.data = newdata
                kexm.updatedata(id, datawithstep)
            end
        }
        for i, step in steps do 
            if step.step == datawithstep.step then atstep = true end -- We got the step
            if atstep then 
                local ok, res = xpcall(step.fn, debug.traceback :: any, cctx)
                if res ~= "OK" then
                    -- error handled silently
                    kexm.remove(id) -- Remove checkpoint on error to prevent getting stuck
                    error(res)
                else 
                    -- Look for next step
                    local nextstep = steps[i + 1]
                    if nextstep then
                        datawithstep.step = nextstep.step -- always update step to next
                        if res ~= nil then datawithstep.data = res end -- update data if step function returned new data
                        kexm.updatedata(id, datawithstep)
                    end
                end
            end
        end

        kexm.remove(id) -- No more steps, remove checkpoint
    end

    local kexm = KeyExpiryManager<<DataWithStep<Data>>>(ctx, "checkpoints/"..baseid, function(record)
        return nil
    end, function(records, kexm) 
        assert(kexm, "Internal error: kexm should not be nil in checkpoint manager callback")
        -- On init, resume any checkpoints
        -- Resuming checkpoints
        for _, record in records do 
            local id = record.key
            local datawithstep = record.value :: DataWithStep<Data>
            task.spawn(loadinner, id, datawithstep, true, kexm, sb.StatusBuffer())
        end
        return nil
    end)
    
    local function new(initialdata: Data, sbuf: sb.StatusBuffer?) 
        -- Implementation goes here
        local id = typesext.randstring(32)
        local currstep = steps[1].step
        local data: DataWithStep<Data> = {
            step = currstep,
            data = initialdata,
        }

        -- Store checkpoint and start the process
        kexm.add(expiry, data, id)
        loadinner(id, data, false, kexm, sbuf or sb.StatusBuffer())
    end

    local self = {}
    self.new = new
    return self
end

return CheckpointManager