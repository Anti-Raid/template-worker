--!strict
type State = "Pending" | "Running" | "Completed" | "Failed"

export type LazyLoader<T, Args> = {
    get: (Args) -> T
}

local function LazyLoader<T, Args>(fn: (Args) -> T): LazyLoader<T, Args>
    local state: State = "Pending"
    local results: T? = nil
    local err: any? = nil
    local queue: {thread} = {}
    
    -- To avoid re-entrancy, track the currently executing thread
    local runningThread: thread? = nil

    local function get(args: Args): T
        -- Fast path: already completed or failed
        if state == "Completed" then
            return results :: T
        elseif state == "Failed" then
            error(err, 2)
        -- If we're already running, wait until we're woken up
        elseif state == "Running" then
            if runningThread == coroutine.running() then
                error("Circular dependency detected: LazyLoader attempted to evaluate itself.", 2)
            end
            table.insert(queue, coroutine.running())
            coroutine.yield()
            
            if state == "Completed" then
                return results :: T
            else
                error(err, 2)
            end
        end

        -- Running the function for the first time
        state = "Running"
        runningThread = coroutine.running()
        local success, res = pcall(fn, args)

        if success then
            state = "Completed"
            results = res
        else
            state = "Failed"
            err = res
        end

        runningThread = nil

        -- Wake up waiting threads
        for _, th in queue do
            task.spawn(th)
        end
        table.clear(queue)

        if state == "Completed" then
            return results :: T
        else
            error(err, 2)
        end
    end

    return {
        get = get
    }
end

return LazyLoader