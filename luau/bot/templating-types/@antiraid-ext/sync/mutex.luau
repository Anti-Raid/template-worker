--!strict
export type MutexFn = {
    --- Calls a function `fn`, waiting if the MutexFn is already in the middle of calling another `fn` currently
    read call: <T>(fn: () -> T) -> T,
    --- Returns true if the mutex is calling `fn`
    read locked: () -> boolean
}

--- @noyield
---
--- Creates a new MutexFn instance that locks yield-based accesses to callbacks
local function MutexFn(): MutexFn
    local queue: {thread} = {}    
    local islocked = false
    -- To avoid re-entrancy, track the currently executing thread
    local runningThread: thread? = nil

    local function call<T>(fn: () -> T): T
        if islocked then
            if runningThread == coroutine.running() then
                error("Circular dependency detected: MutexFn attempted to evaluate itself.", 2)
            end
            table.insert(queue, coroutine.running())
            coroutine.yield() -- Wait until the current execution finishes
        else
            -- Acquire the lock since no one else is currently executing
            islocked = true
            runningThread = coroutine.running()
        end

        local success, res = xpcall(fn, function(res) return debug.traceback(tostring(res)) end)

        -- Wake up next thread in the queue
        local nextThread = table.remove(queue, 1)
        if nextThread then
            -- Pass the lock to the next thread
            runningThread = nextThread
            task.spawn(nextThread)
        else 
            -- No more threads waiting, release the lock
            islocked = false
            runningThread = nil
        end

        if success then
            return res
        else
            error(res, 1)
        end
    end

    local function locked()
        return islocked
    end

    return table.freeze{
        call = call,
        locked = locked
    }
end

return MutexFn