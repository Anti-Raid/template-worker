local concurrency  = require"./concurrency"
local setTimeout = concurrency.setTimeout
local clearTimeout = concurrency.clearTimeout
local setInterval = concurrency.setInterval
local clearInterval = concurrency.clearInterval

setTimeout(function()
    print("Task")
end, 1000)

print("Hello, world A")

setTimeout(function()
    print("Task2")
end, 2000)

print("Hello, world B")

setTimeout(function()
    print("Task3")
end, 500)

local idx = 0
local tid;
tid = setInterval(function()
    idx += 1
    print(idx)

    if(idx >= 5) then
        clearInterval(tid :: number)
    end
end, 1000)

while(idx < 5) do
    task.wait(0.1)
end

print("Hello, world C")

local sidABC = setInterval(function()
    print("This should not print")
end, 10000)

setTimeout(function()
    print("Clearing interval for sidABC")
    clearInterval(sidABC)
end, 100)

print("Hello, world D")

local tidABC = setTimeout(function()
    print("This should also not print")
end, 10000)

setTimeout(function()
    print("Clearing timeout for tidABC")
    clearTimeout(tidABC)
end, 1000)
