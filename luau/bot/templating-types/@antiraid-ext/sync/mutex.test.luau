-- Generated with help from Gemini
local MutexFn = require"./mutex"

local function runTests()
    print("Starting MutexFn Tests...\n")
    
    -- ==========================================
    -- TEST 1: Mutual Exclusion (No Overlapping)
    -- ==========================================
    print("Test 1: Checking Mutual Exclusion...")
    local m1 = MutexFn()
    local concurrentExecutions = 0
    local maxConcurrent = 0
    local completedTasks = 0

    for i = 1, 5 do
        task.spawn(function()
            m1.call(function()
                -- Track how many threads are inside this block at the same time
                concurrentExecutions += 1
                if concurrentExecutions > maxConcurrent then 
                    maxConcurrent = concurrentExecutions 
                end
                
                -- Yielding here tempts other threads to jump in if the lock is broken
                task.wait(0.1) 
                
                concurrentExecutions -= 1
                completedTasks += 1
                return nil
            end)
        end)
    end

    -- Wait long enough for all 5 tasks to finish (5 * 0.1s = 0.5s + buffer)
    task.wait(1)
    
    assert(maxConcurrent == 1, `FAIL: Mutex allowed {maxConcurrent} concurrent executions!`)
    assert(completedTasks == 5, `FAIL: Only {completedTasks}/5 tasks completed.`)
    print("✔ Test 1 Passed: Threads ran strictly one at a time.\n")

    -- ==========================================
    -- TEST 2: Return Values
    -- ==========================================
    print("Test 2: Checking Return Values...")
    local m2 = MutexFn()
    
    local result = m2.call(function()
        return "Secure Data"
    end)

    assert(result == "Secure Data", "FAIL: Mutex did not return the correct value.")
    print("✔ Test 2 Passed: Values returned successfully.\n")

    -- ==========================================
    -- TEST 3: Error Recovery (Lock doesn't jam)
    -- ==========================================
    print("Test 3: Checking Error Recovery...")
    local m3 = MutexFn()
    local didThread2Run = false

    -- Thread 1: Intentionally crashes
    task.spawn(function()
        local success = pcall(function()
            m3.call(function()
                if false then return nil end
                error("Simulated crash!")
            end)
        end)
        assert(not success, "FAIL: Error was swallowed by the Mutex.")
    end)

    -- Thread 2: Tries to run immediately after the crash
    task.spawn(function()
        m3.call(function()
            didThread2Run = true
            return nil
        end)
    end)

    task.wait(0.1)
    assert(didThread2Run, "FAIL: Mutex remained locked after the first thread errored.")
    print("✔ Test 3 Passed: Lock successfully recovered from an error.\n")

    -- ==========================================
    -- TEST 4: Re-entrancy / Deadlock Detection
    -- ==========================================
    print("Test 4: Checking Deadlock Prevention...")
    local m4 = MutexFn()
    local caughtDeadlock = false

    m4.call(function()
        local success, err = pcall(function()
            -- Attempting to call the exact same Mutex from inside itself
            m4.call(function() 
                print("This should never print.") 
                return nil
            end)
            return nil
        end)
        
        if not success and string.find(tostring(err), "Circular dependency detected") then
            caughtDeadlock = true
        end

        return nil
    end)

    assert(caughtDeadlock, "FAIL: Mutex allowed a thread to deadlock itself without throwing the correct error.")
    print("✔ Test 4 Passed: Circular dependency safely caught.\n")

    print("All MutexFn tests passed successfully! 🎉")
end

-- Run the test suite
runTests()