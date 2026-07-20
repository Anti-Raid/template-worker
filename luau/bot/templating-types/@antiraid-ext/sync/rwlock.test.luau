--!strict
--
-- Generated with help from Gemini
local ReadWriteFn = require("./rwlock")

local function runRwLockTests()
    print("--- Running ReadWriteFn Tests ---")
    local rwlock = ReadWriteFn()

    -- ==========================================
    -- TEST 1: Multiple Concurrent Readers
    -- ==========================================
    print("Test 1: Multiple Concurrent Readers")
    local concurrentReaders = 0
    local maxConcurrentReaders = 0
    local completedReaders = 0

    for i = 1, 5 do
        task.spawn(function()
            rwlock.call(function()
                concurrentReaders += 1
                if concurrentReaders > maxConcurrentReaders then
                    maxConcurrentReaders = concurrentReaders
                end
                task.wait(0.1) -- Yield to let other readers enter the lock simultaneously
                concurrentReaders -= 1
                return nil
            end)
            completedReaders += 1
        end)
    end

    task.wait(0.2) -- Wait for all to finish
    
    assert(completedReaders == 5, "Not all readers finished!")
    assert(maxConcurrentReaders == 5, `Expected 5 readers to run concurrently, but only got {maxConcurrentReaders}. The read lock is incorrectly blocking!`)
    assert(not rwlock.locked(), "Lock remained active after all readers finished.")
    print("  ✓ Passed")


    -- ==========================================
    -- TEST 2: Mutual Exclusion (Writers Block Everything)
    -- ==========================================
    print("Test 2: Mutual Exclusion (Writer Isolation)")
    local sharedValue = 0
    local writerRan = false

    -- Lock it with a writer
    task.spawn(function()
        rwlock.callmut(function()
            task.wait(0.2) -- Hold the write lock
            sharedValue = 42
            writerRan = true
            return nil
        end)
    end)

    task.wait(0.05) -- Ensure writer has acquired the lock

    -- Spawn a reader. It should NOT be able to read until the writer finishes.
    local readValue = -1
    task.spawn(function()
        rwlock.call(function()
            readValue = sharedValue
            return nil
        end)
    end)

    task.wait(0.05)
    assert(readValue == -1, "Fatal: Reader managed to read data while a Writer held the lock!")
    
    task.wait(0.2) -- Wait for writer to finish and reader to wake up
    assert(writerRan, "Writer never finished.")
    assert(readValue == 42, "Reader did not read the correct value after writer finished.")
    print("  ✓ Passed")


    -- ==========================================
    -- TEST 3: Writer Preference (Anti-Starvation)
    -- ==========================================
    print("Test 3: Writer Preference")
    local executionOrder = {}

    -- 1. Start a slow reader
    task.spawn(function()
        rwlock.call(function()
            table.insert(executionOrder, "Reader 1 Start")
            task.wait(0.3)
            table.insert(executionOrder, "Reader 1 End")
            return nil
        end)
    end)

    task.wait(0.05) -- Ensure Reader 1 is holding the lock

    -- 2. Queue a writer (it will wait for Reader 1)
    task.spawn(function()
        rwlock.callmut(function()
            table.insert(executionOrder, "Writer")
            return nil
        end)
    end)

    task.wait(0.05) -- Ensure Writer is in the queue

    -- 3. Queue another reader
    -- IMPORTANT: Because a writer is waiting, this reader MUST wait for the writer to finish,
    -- even though Reader 1 currently holds a read lock.
    task.spawn(function()
        rwlock.call(function()
            table.insert(executionOrder, "Reader 2")
            return nil
        end)
    end)

    task.wait(0.4) -- Let everything resolve

    -- Verify the strict order
    assert(executionOrder[1] == "Reader 1 Start", "Test timing failed")
    assert(executionOrder[2] == "Reader 1 End", "Test timing failed")
    assert(executionOrder[3] == "Writer", "Writer preference failed! Reader 2 jumped ahead of the waiting writer.")
    assert(executionOrder[4] == "Reader 2", "Reader 2 did not execute after the writer.")
    print("  ✓ Passed")


    -- ==========================================
    -- TEST 4: Error Handling & State Recovery
    -- ==========================================
    print("Test 4: Error Unwinding")
    
    -- Test Read Error
    local ok1 = pcall(function()
        rwlock.call(function() 
            error("Read crash", 0) 
            return nil 
        end)
    end)
    assert(not ok1, "Expected read to throw")
    assert(not rwlock.locked(), "Read lock did not release after crash!")

    -- Test Write Error
    local ok2 = pcall(function()
        rwlock.callmut(function() 
            error("Write crash", 0) 
            return nil
        end)
    end)
    assert(not ok2, "Expected write to throw")
    assert(not rwlock.locked(), "Write lock did not release after crash!")
    print("  ✓ Passed")


    -- ==========================================
    -- TEST 5: Deadlock / Re-entrancy Prevention
    -- ==========================================
    print("Test 5: Deadlock Prevention")
    
    -- Scenario A: Writer tries to read
    local okA, errA = pcall(function()
        rwlock.callmut(function()
            rwlock.call(function() return nil end)
            return nil
        end)
        return nil
    end)
    assert(not okA, "Writer successfully downgraded to reader, which is forbidden.")
    assert(string.find(tostring(errA), "Circular dependency"), "Wrong error for Writer->Reader deadlock")

    -- Scenario B: Writer tries to write again
    local okB, errB = pcall(function()
        rwlock.callmut(function()
            rwlock.callmut(function() return nil end)
            return nil
        end)
        return nil
    end)
    assert(not okB, "Writer successfully re-acquired write lock, which is forbidden.")
    assert(string.find(tostring(errB), "Circular dependency"), "Wrong error for Writer->Writer deadlock")

    -- Scenario C: Reader tries to upgrade to writer
    local okC, errC = pcall(function()
        rwlock.call(function()
            rwlock.callmut(function() return nil end)
            return nil
        end)
        return nil
    end)
    assert(not okC, "Reader successfully upgraded to writer, which is a deadlock trap.")
    assert(string.find(tostring(errC), "Deadlock detected"), "Wrong error for Reader->Writer deadlock")

    print("  ✓ Passed")

    print("--- All ReadWriteFn Tests Passed! ---")
end

runRwLockTests()