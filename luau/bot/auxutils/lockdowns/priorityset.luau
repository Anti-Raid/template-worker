--- @internal Lockdown only
--- Priority sets
---
--- PrioritySet can add elements with its priority with all priorities being saved.
--- When removing an element, the elements prior priority will be set.
---
--- **Usage notes**
---
--- 1. The elements stored must be hashable (e.g. strings, numbers, etc.).
export type PrioritySet<K> = typeof(setmetatable({} :: {
    --- Adds an element to the priority set with priority
    add: (element: K, priority: number) -> (),
    --- Removes an element from the priority set with priority
    remove: (element: K, priority: number) -> (),
    --- Returns the highest priority given element
    highest_priority: (element: K) -> number?,
    --- Returns the lowest priority given element
    lowest_priority: (element: K) -> number?,
    --- Returns if an element exists in the priority set or not
    exists: (element: K) -> boolean
}, {} :: {
    __iter: (self: PrioritySet<K>) -> () -> (K?, number?)
}))

local function PrioritySet<K>(): PrioritySet<K>
    local self = {}

    local elements: {[K]: {number}} = {}

    local function add(element: K, priority: number)
        if typeof(element) ~= "string" and typeof(element) ~= "number" then 
            error("PrioritySet can only be called using string and number arguments")
        end

        if not elements[element] then
            elements[element] = {}
        end
        table.insert(elements[element], priority)
    end

    local function remove(element: K, priority: number)
        if elements[element] then
            local new_priorities = {}
            local removed = false

            for i = #elements[element], 1, -1 do
                local p = elements[element][i]
                if p == priority and not removed then
                    removed = true
                else
                    table.insert(new_priorities, 1, p)
                end
            end

            if #new_priorities == 0 then
                elements[element] = nil
            else
                elements[element] = new_priorities
            end
        end
    end

    local function highest_priority(element: K): number | nil
        if elements[element] then
            local max_priority = nil
            for _, p in elements[element] do
                if not max_priority or p > max_priority then
                    max_priority = p
                end
            end
            return max_priority
        end
        return nil
    end

    local function lowest_priority(element: K): number | nil
        if elements[element] then
            local min_priority = nil
            for _, p in elements[element] do
                if not min_priority or p < min_priority then
                    min_priority = p
                end
            end
            return min_priority
        end
        return nil
    end

    local function exists(element: K): boolean
        return elements[element] ~= nil
    end

    -- Should return iterator function and state as per Luau docs for generalized iterators.
    setmetatable(self, {
        --- Iterates over the elements and their highest priorities
        __iter = function()
            local index = 0
            local keys: {K} = {}
            for k, _ in elements do
                table.insert(keys, k)
            end

            return function(): (K?, number?)
                index = index + 1
                if index <= #keys then
                    local key = keys[index]
                    local max_priority = highest_priority(key)
                    return key, max_priority
                end
                return nil, nil
            end
        end,
    })

    self.add = add
    self.remove = remove
    self.highest_priority = highest_priority
    self.lowest_priority = lowest_priority
    self.exists = exists

    return self :: PrioritySet<K>
end

return PrioritySet