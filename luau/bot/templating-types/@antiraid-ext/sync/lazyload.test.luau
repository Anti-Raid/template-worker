-- Generated with help of Gemini
local LazyLoader = require"./lazyload"

local function runTests()
    print("--- Running LazyLoader Tests ---")

    -- ==========================================
    -- TEST 1: Basic Evaluation & Caching
    -- ==========================================
    print("Test 1: Basic Evaluation & Caching")
    local count1 = 0
    local loader1 = LazyLoader(function()
        count1 += 1
        return "Success" 
    end)

    -- First call (Execution Path)
    local res1 = loader1.get()
    assert(res1 == "Success", "Failed to get initial values")
    assert(count1 == 1, "Execution count should be 1")

    -- Second call (Fast Path)
    local res2 = loader1.get()
    assert(res2 == "Success", "Failed to get cached values")
    assert(count1 == 1, "Execution count increased! The function ran twice.")
    print("  ✓ Passed")


    -- ==========================================
    -- TEST 2: Concurrency & Yielding
    -- ==========================================
    print("Test 2: Concurrency & Yielding (Wait Path)")
    local count2 = 0
    local loader2 = LazyLoader(function()
        count2 += 1
        task.wait(0.1) -- Simulate an expensive async operation (e.g., file read)
        return "AsyncData"
    end)

    local results = {}
    local completedThreads = 0

    -- Unleash 5 threads at the exact same time
    for i = 1, 10 do
        task.spawn(function()
            local res = loader2.get()
            results[i] = res
            completedThreads += 1
        end)
    end

    -- Wait long enough for the loader's task.wait(0.1) to finish
    task.wait(0.2)

    assert(completedThreads == 10, "Not all yielded threads were woken up!")
    assert(count2 == 1, `Race condition! Function was executed {count2} times concurrently!`)
    
    for i = 1, 5 do
        assert(results[i] == "AsyncData", `Thread {i} got wrong data: {tostring(results[i])}`)
    end
    print("  ✓ Passed")


    -- ==========================================
    -- TEST 3: Error Caching
    -- ==========================================
    print("Test 3: Error Caching")
    local count3 = 0
    local loader3 = LazyLoader(function()
        if false then return "" end -- for typing
        count3 += 1
        error("SyntaxError in init.luau", 0)
    end)

    -- First call (Execution Path)
    local ok1, err1 = pcall(function() loader3.get() end)
    assert(not ok1, "Expected loader to throw an error, but it succeeded.")
    assert(string.find(tostring(err1), "SyntaxError"), "Threw wrong error: " .. tostring(err1))
    assert(count3 == 1, "Execution count should be 1")

    -- Second call (Fast Path Error Retrieval)
    local ok2, err2 = pcall(function() loader3.get() end)
    assert(not ok2, "Expected cached loader to throw an error, but it succeeded.")
    assert(string.find(tostring(err2), "SyntaxError"), "Threw wrong cached error: " .. tostring(err2))
    assert(count3 == 1, "Execution count increased! The failing function ran twice.")
    print("  ✓ Passed")

    -- ==========================================
    -- TEST 5: Circular Dependency (Deadlock Prevention)
    -- ==========================================
    print("Test 5: Circular Dependency")
    local loader5
    loader5 = LazyLoader(function()
        -- The function tries to call itself while it is already running
        if loader5 then loader5.get() end
        return ""
    end)

    local ok, err = pcall(function() 
        if not loader5 then error("abc") end
        return loader5.get() 
    end)
    assert(not ok, "Expected circular dependency to throw an error, but it yielded forever!")
    assert(string.find(tostring(err), "Circular dependency"), "Threw wrong error: " .. tostring(err))
    print("  ✓ Passed")

    print("--- All Tests Passed! ---")
end

-- Execute the test suite
runTests()