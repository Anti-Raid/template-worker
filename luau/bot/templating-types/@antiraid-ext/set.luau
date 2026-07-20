--- Sets
---
--- Sets can add/get unique elements.
---
--- **Usage notes**
---
--- 1. The elements stored must be hashable (e.g. strings, numbers, etc.).
export type Set<K> = typeof(setmetatable({} :: {
    --- Adds an element to the set
    add: (element: K) -> (),
    --- Inserts a set of elements into the set
    insert: (elements: {K}) -> (),
    --- Removes an element from the set 
    remove: (element: K) -> (),
    --- Deletes a set of elements from the set
    delete: (elements: {K}) -> (),
    --- Returns if an element exists in the priority set or not
    exists: (element: K) -> boolean,
    --- Clears the set
    clear: () -> (),
    --- Copy of the set
    --- 
    --- **Time complexity: O(n)** where `n` is the number of elements in the set.
    copy: () -> Set<K>,
}, {} :: {
    __iter: (self: Set<K>) -> () -> K?,
    __len: (self: Set<K>) -> number,
}))

local function Set<K>(): Set<K>
    local self = {}

    local elements: {[K]: true} = {}
    local count = 0 -- Number of elements in the set

    local function add(element: K)
        if typeof(element) ~= "string" and typeof(element) ~= "number" then 
            error("Set can only be called using string and number arguments")
        end

        if elements[element] then return end -- Already exists, no need to add again

        elements[element] = true -- Add the element to the set
        count += 1
    end

    local function insert(elements: {K}) 
        for _, element in elements do add(element) end
    end

    local function remove(element: K)
        if not elements[element] then return end -- Element does not exist, no need to remove
        elements[element] = nil
        count -= 1
    end

    local function delete(elements: {K}) 
        for _, element in elements do remove(element) end
    end

    local function exists(element: K): boolean
        return elements[element] ~= nil
    end

    local function clear()
        elements = {}
        count = 0
    end

    local function copy(): Set<K>
        local newSet = Set()
        for element in elements do
            newSet.add(element)
        end
        return newSet
    end

    -- Should return iterator function and state as per Luau docs for generalized iterators.
    setmetatable(self, {
        --- Iterates over the elements and their highest priorities
        __iter = function()
            return function(k: {[K]: boolean}, v: K?): K?
                local nextEl, _ = next(k, v)
                return nextEl
            end, elements
        end,
        --- Returns the number of elements in the set
        __len = function()
            return count
        end,
    })

    self.add = add
    self.insert = insert
    self.remove = remove
    self.delete = delete
    self.exists = exists
    self.clear = clear
    self.copy = copy

    return self :: Set<K>
end

--- Creates a set from a table of elements
---
--- **Time complexity: O(n)** where `n` is the number of elements in the table.
local function fromtable<K>(table: {K}): Set<K>
    local Set = Set()
    for _, element in table do
        Set.add(element)
    end

    return Set
end

return {
    Set = Set,
    fromtable = fromtable,
}