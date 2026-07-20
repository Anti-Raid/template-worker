--!strict
--
-- Code created with help of Gemini

export type ReadWriteFn = {
    --- Calls a function `fn` as a reader. Multiple readers can execute simultaneously.
    read call: <T>(fn: () -> T) -> T,
    
    --- Calls a function `fn` as a writer. Only one writer can execute at a time.
    read callmut: <T>(fn: () -> T) -> T,
    
    --- Returns true if the lock is currently held by any reader or writer.
    read locked: () -> boolean
}

local function ReadWriteFn(): ReadWriteFn
    local readQueue: {thread} = {}
    local writeQueue: {thread} = {}
    
    local readers = 0
    local writerActive = false
    
    -- Tracking to prevent deadlocks and re-entrancy
    local activeWriterThread: thread? = nil
    local activeReaderThreads: {[thread]: boolean} = {}

    local function wakeNext()
        if writerActive or readers > 0 then
            return -- The lock is still in use
        end

        -- Writer Preference: Always wake up a waiting writer first
        if #writeQueue > 0 then
            writerActive = true
            activeWriterThread = table.remove(writeQueue, 1)
            task.spawn(activeWriterThread :: thread)
            
        -- If no writers are waiting, wake up ALL waiting readers simultaneously
        elseif #readQueue > 0 then
            local toWake = readQueue
            readQueue = {} -- Clear the queue
            readers = #toWake
            
            for _, th in toWake do
                activeReaderThreads[th] = true
                task.spawn(th)
            end
        end
    end

    local function call<T>(fn: () -> T): T
        local currentThread = coroutine.running()
        
        -- DEADLOCK PREVENTION: Must be checked globally before any yielding logic
        if activeWriterThread == currentThread then
            error("Circular dependency detected: active writer attempted to acquire read lock.", 2)
        end

        -- Wait if a writer is active, OR if writers are waiting (Writer Preference)
        if writerActive or #writeQueue > 0 then
            table.insert(readQueue, currentThread)
            coroutine.yield() 
        else
            readers += 1
            activeReaderThreads[currentThread] = true
        end

        -- Execute the function safely
        local success, res = xpcall(fn, function(err) return debug.traceback(tostring(err)) end)

        -- Release Read Lock
        readers -= 1
        activeReaderThreads[currentThread] = nil
        
        -- If we were the last reader, check if a writer needs to be woken up
        if readers == 0 then
            wakeNext()
        end

        if success then
            return res
        else
            error(res, 1)
        end
    end

    local function callmut<T>(fn: () -> T): T
        local currentThread = coroutine.running()
        
        -- DEADLOCK PREVENTION
        if activeWriterThread == currentThread then
            error("Circular dependency detected: active writer attempted to re-acquire write lock.", 2)
        end
        if activeReaderThreads[currentThread] then
            error("Deadlock detected: active reader attempted to upgrade to write lock.", 2)
        end

        -- Wait if ANY readers or writers are active
        if writerActive or readers > 0 then
            table.insert(writeQueue, currentThread)
            coroutine.yield()
        else
            writerActive = true
            activeWriterThread = currentThread
        end

        -- Execute the function safely
        local success, res = xpcall(fn, function(err) return debug.traceback(tostring(err)) end)

        -- Release Write Lock
        writerActive = false
        activeWriterThread = nil
        
        -- Wake up the next writer, or all waiting readers
        wakeNext()

        if success then
            return res
        else
            error(res, 1)
        end
    end

    local function locked()
        return writerActive or readers > 0
    end

    return table.freeze{
        call = call,
        callmut = callmut,
        locked = locked
    }
end

return ReadWriteFn