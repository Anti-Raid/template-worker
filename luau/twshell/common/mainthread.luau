export type Task = {
    __phantom: "Task",
}

export type MainThread = {
    AddTask: (self: MainThread, task: Task) -> any,
}

return {}