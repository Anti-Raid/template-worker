local Queue = {}
Queue.__index = Queue

--- A simple queue implementation in Luau.
export type Queue<T> = typeof(setmetatable({} :: {
    _first: number,
    _last: number,
    _queue: { T },
    isEmpty: (self: Queue<T>) -> boolean,
    enqueue: (self: Queue<T>, value: T) -> (),
    dequeue: (self: Queue<T>) -> T,
}, {} :: { }))

--- Create a new queue
function Queue.new<T>(): Queue<T>
	local self = setmetatable({
		_first = 0,
		_last = -1,
		_queue = {},
	}, Queue)

	return self :: Queue<T>
end

-- Check if the queue is empty
function Queue.isEmpty<T>(self: Queue<T>)
	return self._first > self._last
end

--- Add a value to the queue
function Queue.enqueue<T>(self: Queue<T>, value: T)
	local last = self._last + 1
	self._last = last
	self._queue[last] = value
end

--- Remove a value from the queue
function Queue.dequeue<T>(self: Queue<T>): T
	if self:isEmpty() then
		error("Cannot dequeue from empty queue")
	end

	local first = self._first
	local value = self._queue[first]
	self._queue[first] = nil
	self._first = first + 1

	return value
end

local Stack = {}
Stack.__index = Stack

--- A simple stack implementation in Luau.
export type Stack<T> = {
    _stack: { T },
    isEmpty: (self: Stack<T>) -> boolean,
    push: (self: Stack<T>, value: T) -> (),
    pop: (self: Stack<T>) -> T,
}

--- Create a new stack
function Stack.new<T>(): Stack<T>
	local self = setmetatable({
        _stack = {},
    }, Stack)

	return self :: Stack<T>
end

--- Check if the stack is empty
function Stack.isEmpty<T>(self: Stack<T>)
	return #self._stack == 0
end

--- Put a new value onto the stack
function Stack.push<T>(self: Stack<T>, value)
	table.insert(self._stack, value)
end

--- Take a value off the stack
function Stack.pop<T>(self: Stack<T>)
	if self:isEmpty() then
		return nil
	end

	return table.remove(self._stack, #self._stack)
end

-- Test
if(_G.cli ~= nil) then
    print("Testing Queue and Stack implementations...")
    local q = Queue.new() :: Queue<number>
    assert(q:isEmpty(), "Queue should be empty initially")
    q:enqueue(1)
    q:enqueue(2)
    q:enqueue(3)
    q:enqueue(4)
    q:enqueue(5)
    assert(not q:isEmpty(), "Queue should not be empty after enqueuing elements")
    assert(q:dequeue() == 1, "Queue dequeue failed [1]")
    assert(q:dequeue() == 2, "Queue dequeue failed [2]")
    assert(q:dequeue() == 3, "Queue dequeue failed [3]")
    assert(q:dequeue() == 4, "Queue dequeue failed [4]")
    assert(q:dequeue() == 5, "Queue dequeue failed [5]")
    assert(q:isEmpty(), "Queue should be empty after dequeuing all elements")
    --q:enqueue("a") -- [type errors as expected]

    local s = Stack.new() :: Stack<number>
    assert(s:isEmpty(), "Stack should be empty initially")
    s:push(1)
    s:push(2)
    s:push(3)
    s:push(4)
    s:push(5)
    assert(not s:isEmpty(), "Stack should not be empty after pushing elements")
    assert(s:pop() == 5, "Stack pop failed [1]")
    assert(s:pop() == 4, "Stack pop failed [2]")
    assert(s:pop() == 3, "Stack pop failed [3]")
    assert(s:pop() == 2, "Stack pop failed [4]")
    assert(s:pop() == 1, "Stack pop failed [5]")
    assert(s:isEmpty(), "Stack should be empty after popping all elements")
    --s:push("a") -- [type errors as expected]
end

return {
    Stack = Stack,
    Queue = Queue,
}