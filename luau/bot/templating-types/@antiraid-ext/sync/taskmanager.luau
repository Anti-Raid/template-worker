--!strict
local concurrency = require"./concurrency"

--- Task Manager is a construct for managing recurring tasks
export type TaskManager = {
    --- Registers a recurring task with the given name and period (in seconds)
    ---
    --- If the task errors, it will be caught and `onerror` will be called with the error message and stack trace.
    read register: (name: string, secs: number, task: () -> ()) -> (),

    --- Cancels a recurring task with the given name
    read cancel: (name: string) -> (),
}

--- Returns a Task Manager which can be used to register and cancel recurring tasks
local function TaskManager(onerror: (name: string, err: any) -> ()): TaskManager
    local tasks: {[string]: {
        secs: number,
        tid: number,
    }} = {}

    local function register(name: string, secs: number, task: () -> ())
        if tasks[name] then
            error(`Task with name {name} is already registered`)
        end

        local tid = concurrency.setInterval(function()
            local ok, err = xpcall(task :: () -> nil, debug.traceback :: any) 
            if not ok then
                onerror(name, err)
            end
        end, secs * 1000)

        tasks[name] = {
            secs = secs,
            tid = tid,
        }
    end

    local function cancel(name: string)
        local taskInfo = tasks[name]
        if not taskInfo then
            error(`Task with name {name} is not registered`)
        end

        concurrency.clearInterval(taskInfo.tid)
        tasks[name] = nil
    end

    return table.freeze{
        register = register,
        cancel = cancel,
    }
end

return TaskManager