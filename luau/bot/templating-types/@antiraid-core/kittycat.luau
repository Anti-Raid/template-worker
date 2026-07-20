--!nocheck

--- Permission type
export type Permission = {
    --- Namespace of the permission
    namespace: string,
    --- Permission of the permission
    perm: string,
    --- Whether the permission is negated
    negator: boolean,
}

local Permission = {}
Permission.__index = Permission

local PartialStaffPosition = {}
PartialStaffPosition.__index = PartialStaffPosition

local StaffPermissions = {}
StaffPermissions.__index = StaffPermissions

-- Permission implementation
function Permission.new(namespace: string, perm: string, negator: boolean): Permission
    local self = setmetatable({}, Permission)
    self.__index = Permission
    self.namespace = namespace
    self.perm = perm
    self.negator = negator
    return self
end

function Permission.from_string(perm_str)
    local negator = string.sub(perm_str, 1, 1) == "~"
    local perm_without_negator = negator and string.sub(perm_str, 2) or perm_str
    
    local namespace, perm = string.match(perm_without_negator, "([^.]+)%.(.+)")
    if not namespace then
        namespace = "global"
        perm = perm_without_negator
    end
    return Permission.new(namespace :: string, perm :: string, negator)
end

function Permission.from_tab(perm: Permission)
    return Permission.new(perm.namespace, perm.perm, perm.negator)
end

function Permission.__tostring(self: Permission)
    return (self.negator and "~" or "") .. self.namespace .. "." .. self.perm
end

--- PartialStaffPosition type
export type PartialStaffPosition = {
    id: string,
    index: number,
    perms: {Permission},
}

-- PartialStaffPosition implementation
function PartialStaffPosition.new(id: string, index: number, perms: {Permission}): PartialStaffPosition
    local self = setmetatable({}, PartialStaffPosition)
    self.id = id
    self.index = index
    self.perms = perms
    return self
end

--- StaffPermissions type
export type StaffPermissions = {
    user_positions: {PartialStaffPosition},
    perm_overrides: {Permission},
    resolve: (self: StaffPermissions) -> {Permission},
}

-- StaffPermissions implementation
function StaffPermissions.new(user_positions: {PartialStaffPosition}, perm_overrides: {Permission}): StaffPermissions
    local self = setmetatable({}, StaffPermissions)
    self.user_positions = user_positions
    self.perm_overrides = perm_overrides
    return self :: StaffPermissions
end

type OrderedStrNumTable = {
    data: {[string]: number},
    order: {string},
    get: (self: OrderedStrNumTable, key: string) -> (),
    insert: (self: OrderedStrNumTable, key: string, value: number) -> (),
    remove: (self: OrderedStrNumTable, key: string) -> (),
    iterByInsertionOrder: (self: OrderedStrNumTable) -> () -> (string, number),
    clear: (self: OrderedStrNumTable) -> ()
}

local OrderedStrNumTable = {}
OrderedStrNumTable.__index = OrderedStrNumTable

function OrderedStrNumTable.new()
    local self = setmetatable({}, OrderedStrNumTable)
    self.__index = OrderedStrNumTable
    self.data = {}
    self.order = {}
    return self :: OrderedStrNumTable
end

function OrderedStrNumTable.get(self: OrderedStrNumTable, key: string)
    return self.data[key]
end

function OrderedStrNumTable.insert(self: OrderedStrNumTable, key: string, value: number)
    if self.data[key] then
        return
    end
    
    self.data[key] = value
    table.insert(self.order, key)
end

function OrderedStrNumTable.remove(self: OrderedStrNumTable, key: string)
    if not self.data[key] then
        return
    end
    
    self.data[key] = nil
    for i, k in ipairs(self.order) do
        if k == key then
            table.remove(self.order, i)
            break
        end
    end
end

function OrderedStrNumTable.iterByInsertionOrder(self: OrderedStrNumTable)
    local i = 0
    return function()
        i += 1
        if i > #self.order then
            return nil
        end
        local key = self.order[i]
        return key, self.data[key]
    end
end

function OrderedStrNumTable.clear(self: OrderedStrNumTable)
    self.data = {}
    self.order = {}
end

function StaffPermissions.resolve(self: StaffPermissions)
    local applied_perms_val: OrderedStrNumTable = OrderedStrNumTable.new()
    
    -- Clone user_positions
    local user_positions: {PartialStaffPosition} = {}
    for _, pos in ipairs(self.user_positions) do
        table.insert(user_positions, pos)
    end
    
    -- Add permission overrides
    table.insert(user_positions, PartialStaffPosition.new(
        "perm_overrides",
        0,
        self.perm_overrides
    ))
    
    -- Sort positions by index in descending order
    table.sort(user_positions, function(a, b)
        -- This needs to return true if first element must come before second element
        return b.index < a.index
    end)
    
    for _, pos in ipairs(user_positions) do
        for _, perm in ipairs(pos.perms) do
            if perm.perm == "@clear" then
                if perm.namespace == "global" then
                    -- Clear all permissions
                    applied_perms_val:clear()
                else
                    -- Clear all perms with this namespace
                    local to_remove: {string} = {}
                    for keystr, _ in applied_perms_val:iterByInsertionOrder() do
                        local key = Permission.from_string(keystr)
                        if key.namespace == perm.namespace then
                            table.insert(to_remove, keystr)
                        end
                    end

                    -- Remove here to avoid immutable borrow
                    for _, key in ipairs(to_remove) do
                        applied_perms_val:remove(key)
                    end
                end

                continue
            end

            local perm_key = tostring(perm)
            if perm.negator then
                -- Check what gave the permission. We *know* its sorted so we don't need to do anything but remove if it exists
                local non_negated = Permission.new(perm.namespace, perm.perm, false)
                local non_negated_key = tostring(non_negated)
                
                if applied_perms_val:get(non_negated_key) then
                    -- Remove old permission
                    applied_perms_val:remove(non_negated_key)
                    -- Add the negator
                    applied_perms_val:insert(perm_key, pos.index)
                else
                    if applied_perms_val:get(perm_key) then
                        -- Case 3: The negator is already applied, so we can ignore 
                        continue
                    end

                    -- Then we can freely add the negator
                    applied_perms_val:insert(perm_key, pos.index)
                end
            else
                -- Special case: If a * element exists for a smaller index, then the negator must be ignored. E.g. manager has ~rpc.PremiumAdd but head_manager has no such negator
                if perm.perm == "*" then
                    -- Remove negators. As the permissions are sorted, we can just check if a negator is in the hashmap
                    
                    -- If the * element is from a permission of lower index, then we can ignore this negator
                    local to_remove: {string} = {}
                    for key, _ in applied_perms_val:iterByInsertionOrder() do
                        local existing_perm = Permission.from_string(key)
                        if not existing_perm.negator then
                            continue -- This special case only applies to negators
                        end

                        -- Same namespaces
                        if existing_perm.namespace == perm.namespace then
                            -- Then we can ignore this negator
                            table.insert(to_remove, key)
                        end
                    end

                    -- Remove here to avoid immutable borrow
                    for _, key in ipairs(to_remove) do
                        applied_perms_val:remove(key)
                    end
                end
                
                -- If its not a negator, first check if there's a negator
                local negated = Permission.new(perm.namespace, perm.perm, true)
                local negated_key = tostring(negated)
                
                if applied_perms_val:get(negated_key) then
                    -- Remove the negator
                    applied_perms_val:remove(negated_key)
                    -- Add the permission
                    applied_perms_val:insert(perm_key, pos.index)
                else
                    if applied_perms_val:get(perm_key) then
                        --  Case 3: The permission is already applied, so we can ignore it
                        continue
                    end

                    -- Then we can freely add the permission
                    applied_perms_val:insert(perm_key, pos.index)
                end
            end
        end
    end
    
    local result = {}
    for perm_str, _ in applied_perms_val:iterByInsertionOrder() do
        table.insert(result, Permission.from_string(perm_str))
    end
    return result
end

-- Helper functions
local function has_perm(perms: {Permission}, perm: Permission)
    local has_perm_val = nil
    local has_negator = false
    
    for _, user_perm in ipairs(perms) do
        if not user_perm.negator and user_perm.namespace == "global" and user_perm.perm == "*" then
            return true
        end
        
        if (user_perm.namespace == perm.namespace or user_perm.namespace == "global") and
           (user_perm.perm == "*" or user_perm.perm == perm.perm) then
            has_perm_val = user_perm
            if user_perm.negator then
                has_negator = true
            end
        end
    end
    
    return has_perm_val ~= nil and not has_negator
end

local function has_perm_str(perms: {string}, perm_str: string)
    local perm: Permission = Permission.from_string(perm_str)
    local converted_perms: {Permission} = {}
    for _, p in ipairs(perms) do
        table.insert(converted_perms, Permission.from_string(p))
    end
    return has_perm(converted_perms, perm)
end

-- Error types
export type CheckPatchChangesError = {
    type: "NoPermission",
    permission: Permission,
} | {
    type: "LacksNegatorForWildcard",
    wildcard: Permission,
    negator: Permission,
}

local function cpce_to_string(err: CheckPatchChangesError?): string
    if not err then
        return "An unknown check_patch_changes error occurred [is nil]"
    end

    if err.type == "NoPermission" then
        return "You do not have permission to change the permission: " .. tostring(err.permission)
    elseif err.type == "LacksNegatorForWildcard" then
        return "You do not have a negator for the wildcard permission: " .. tostring(err.wildcard) .. ", missing negator: " .. tostring(err.negator)
    else
        return "An unknown check_patch_changes error occurred"
    end
end

local function check_patch_changes(manager_perms: {Permission}, current_perms: {Permission}, new_perms: {Permission}): (boolean, CheckPatchChangesError?)
    -- Find symmetric difference between current_perms and new_perms
    local changed: {Permission} = {}

    for _, perm in ipairs(current_perms) do
        local found = false
        for _, new_perm in ipairs(new_perms) do
            if tostring(perm) == tostring(new_perm) then
                found = true
                break
            end
        end
        if not found then
            table.insert(changed, perm)
        end
    end

    for _, perm in ipairs(new_perms) do
        local found = false
        for _, current_perm in ipairs(current_perms) do
            if tostring(perm) == tostring(current_perm) then
                found = true
                break
            end
        end
        if not found then
            table.insert(changed, perm)
        end
    end
    
    for _, perm in ipairs(changed) do
        local resolved_perm = Permission.new(perm.namespace, perm.perm, false)
         
        if not has_perm(manager_perms, resolved_perm) then
            return false, {type = "NoPermission", permission = resolved_perm} :: CheckPatchChangesError
        end
        
        if perm.perm == "*" then
            -- Ensure that new_perms has *at least* negators that manager_perms has within the namespace
            for _, manager_perm in pairs(manager_perms :: {Permission}) do
                if manager_perm.negator and manager_perm.namespace == perm.namespace then
                    local has_negator = false
                    for _, new_perm in ipairs(new_perms) do
                        if tostring(new_perm) == tostring(manager_perm) then
                            has_negator = true
                            break
                        end
                    end
                    if not has_negator then
                        return false, {type = "LacksNegatorForWildcard", wildcard = perm, negator = manager_perm} :: CheckPatchChangesError
                    end
                end
            end
        end
    end
    
    return true, nil
end

local function check_patch_changes_str(manager_perms: {string}, current_perms: {string}, new_perms: {string})
    local converted_manager = {}
    local converted_current = {}
    local converted_new = {}
    
    for _, p in ipairs(manager_perms) do
        table.insert(converted_manager, Permission.from_string(p))
    end
    for _, p in ipairs(current_perms) do
        table.insert(converted_current, Permission.from_string(p))
    end
    for _, p in ipairs(new_perms) do
        table.insert(converted_new, Permission.from_string(p))
    end
    
    return check_patch_changes(converted_manager, converted_current, converted_new)
end

-- Test
local function test()    
    local arraysEqualImpl = require"@antiraid-ext/array".arraysEqual
    local arraysEqual = function(a: any, b: any)
        local is_equal = arraysEqualImpl(a, b)
    
        -- If not equal, print contents of each
        if not is_equal then
            print("Got:")
            for k, v in pairs(a) do
                print(k, v)
            end
            print("Expected:")
            for k, v in pairs(b) do
                print(k, v)
            end
        end

        return is_equal
    end

    -- Test code for has_perm below
    assert(has_perm_str({"global.*"}, "test") == true, "global.* implies test[.*]")
    assert(has_perm_str({"rpc.*"}, "global.*") == false, "rpc.* does not imply global.* as rpc != global")
    assert(has_perm_str({"global.test"}, "rpc.test") == true, "global.test implies rpc.test")
    assert(has_perm_str({"global.test"}, "rpc.view_bot_queue") == false, "global.test does not imply rpc.view_bot_queue as global = rpc, test != view_bot_queue")
    assert(has_perm_str({"global.*"}, "rpc.view_bot_queue") == true, "global.* implies rpc.view_bot_queue")
    assert(has_perm_str({"rpc.*"}, "rpc.ViewBotQueue") == true, "rpc.* implies rpc.view_bot_queue")
    assert(has_perm_str({"rpc.BotClaim"}, "rpc.ViewBotQueue") == false, "rpc.BotClaim does not implies rpc.ViewBotQueue as BotClaim != ViewBotQueue")
    assert(has_perm_str({"apps.*"}, "rpc.ViewBotQueue") == false, "apps.* does not imply rpc.ViewBotQueue, apps != rpc")
    assert(has_perm_str({"apps.*"}, "rpc.*") == false, "apps.* does not imply rpc.*, apps != rpc despite the global permission")
    assert(has_perm_str({"apps.test"}, "rpc.test") == false, "apps.test does not imply rpc.test, apps != rpc despite the permissions being the same")

    -- Negator tests
    assert(has_perm_str({"apps.*"}, "apps.test") == true, "apps.* implies apps.test")
    assert(has_perm_str({"~apps.*"}, "apps.test") == false, "~apps.* does not imply apps.test as it is negated")
    assert(has_perm_str({"apps.*", "~apps.test"}, "apps.test") == false, "apps.* does not imply apps.test due to negator ~apps.test")
    assert(has_perm_str({"~apps.test", "apps.*"}, "apps.test") == false, "apps.* does not imply apps.test due to negator ~apps.test. Same as above with different order of perms to test for ordering")
    assert(has_perm_str({"apps.test"}, "apps.test") == true, "~apps.* does not imply apps.test as it is negated")
    assert(has_perm_str({"apps.test", "apps.*"}, "apps.test") == true, "More tests")
    assert(has_perm_str({"~apps.test", "global.*"}, "apps.test") == true, "Test for global.* handling as a wildcard 'return true'")

    -- Test code for resolve_perms below
    assert(arraysEqual(StaffPermissions.new({}, {Permission.from_string("rpc.test")}):resolve(),  {Permission.from_string("rpc.test")}), "Test for basic resolution of overrides")
    assert(arraysEqual(StaffPermissions.new({PartialStaffPosition.new("test", 1, {Permission.from_string("rpc.test")})}, {}):resolve(),  {Permission.from_string("rpc.test")}), "Test for basic resolution of single position")
    assert(arraysEqual(
        StaffPermissions.new(
            {
                PartialStaffPosition.new("test", 1, {Permission.from_string("rpc.test")}), 
                PartialStaffPosition.new("test2", 2, {Permission.from_string("rpc.test2")})}, 
                {}
            ):resolve(), {
                Permission.from_string("rpc.test2"), Permission.from_string("rpc.test")
            }
        ), 
        "Test for basic resolution of multiple positions"
    )
    assert(arraysEqual(
        StaffPermissions.new({
                PartialStaffPosition.new(
                    "test", 
                    1, 
                    {
                        Permission.from_string("rpc.test"), 
                        Permission.from_string("rpc.test2")
                    }
                ), 
                PartialStaffPosition.new(
                    "test2", 
                    2, 
                    {
                        Permission.from_string("~rpc.test"), 
                        Permission.from_string("~rpc.test3")
                    }
                )
            },
            {}
        ):resolve(),  
        {Permission.from_string("~rpc.test3"), Permission.from_string("rpc.test"), Permission.from_string("rpc.test2")}
    ), "Test for basic resolution of multiple positions with negators")
    assert(arraysEqual(
        StaffPermissions.new(
            {
                PartialStaffPosition.new(
                    "test", 
                    1, 
                    {Permission.from_string("~rpc.test"), Permission.from_string("rpc.test2")}
                ), 
                PartialStaffPosition.new(
                    "test2", 
                    2, 
                    {Permission.from_string("~rpc.test3"), Permission.from_string("rpc.test")}
                )
        }, {}):resolve(),  
        {Permission.from_string("~rpc.test3"), Permission.from_string("~rpc.test"), Permission.from_string("rpc.test2")}
    ), "Same as above but testing negator ordering")
    assert(arraysEqual(
        StaffPermissions.new(
            {
                PartialStaffPosition.new(
                    "test", 
                    1, 
                    {Permission.from_string("~rpc.test"), Permission.from_string("rpc.test2"), Permission.from_string("rpc.test3")}
                ), 
                PartialStaffPosition.new(
                    "test2", 
                    2, 
                    {Permission.from_string("~rpc.test3"), Permission.from_string("~rpc.test2")}
                )
            }, 
            {Permission.from_string("rpc.test")}
        ):resolve(),  
        {Permission.from_string("rpc.test2"), Permission.from_string("rpc.test3"), Permission.from_string("rpc.test")}
    ), "Now mix everything together")
    assert(arraysEqual(
        StaffPermissions.new(
            {
                PartialStaffPosition.new(
                    "test", 
                    1, 
                    {Permission.from_string("~rpc.test"), Permission.from_string("rpc.test2")}
                ), 
                PartialStaffPosition.new(
                    "test", 
                    1, 
                    {Permission.from_string("global.@clear"), Permission.from_string("~rpc.test"), Permission.from_string("rpc.test2")}
                ), 
                PartialStaffPosition.new(
                    "test2", 
                    2, 
                    {Permission.from_string("~rpc.test3"), Permission.from_string("~rpc.test2")}
                )
            }, 
            {Permission.from_string("~rpc.test"), Permission.from_string("rpc.test2"), Permission.from_string("rpc.test3")}
        ):resolve(),  
        {Permission.from_string("~rpc.test"), Permission.from_string("rpc.test2"), Permission.from_string("rpc.test3")}
    ), "@clear")
    assert(arraysEqual(
        StaffPermissions.new({PartialStaffPosition.new("test", 1, {Permission.from_string("rpc.*")}), PartialStaffPosition.new("test2", 2, {Permission.from_string("~rpc.test3"), Permission.from_string("~rpc.test2")})}, {}):resolve(),  
        {Permission.from_string("rpc.*")}
    ), "Special case of * with negators")
    assert(arraysEqual(
        StaffPermissions.new({PartialStaffPosition.new("test2", 1, {Permission.from_string("~rpc.test3"), Permission.from_string("~rpc.test2")}), PartialStaffPosition.new("test", 2, {Permission.from_string("rpc.*")})}, {}):resolve(),  
        {Permission.from_string("rpc.*"), Permission.from_string("~rpc.test3"), Permission.from_string("~rpc.test2")}
    ), "Ensure special case does not apply when index is higher (2 > 1 in the below)")
    assert(arraysEqual(
        StaffPermissions.new({}, {Permission.from_string("~rpc.Claim")}):resolve(),  
        {Permission.from_string("~rpc.Claim")}
    ), "Some common cases. Ensure special case does not apply when index is higher (2 > 1 in the below)")

    -- Test code for check_patch_changes below
    assert(check_patch_changes_str({"global.*"}, {"rpc.test"}, {"rpc.test", "rpc.test2"}), "global.* should always be allowed to change perms")
    assert(not check_patch_changes_str({"rpc.*"}, {"global.*"}, {"rpc.test", "rpc.test2"}), "Users should not be able to remove perms they dont have")
    assert(check_patch_changes_str({"rpc.*"}, {"rpc.test"}, {"rpc.test", "rpc.test2"}), "Basic real world test")
    assert(not check_patch_changes_str({"~rpc.test", "rpc.*"}, {"rpc.foobar"}, {"rpc.*"}), "If adding '*' permissions, target must have all negators of user")
    assert(check_patch_changes_str({"~rpc.test", "rpc.*"}, {"~rpc.test"}, {"rpc.*", "~rpc.test", "~rpc.test2"}), "This is OK though")
    assert(not check_patch_changes_str({"~rpc.test", "rpc.*"}, {"~rpc.test"}, {"rpc.*", "~rpc.test2", "~rpc.test2"}), "But this isnt as rpc.test negator isnt in new")
end

return {
    Permission = Permission,
    PartialStaffPosition = PartialStaffPosition,
    StaffPermissions = StaffPermissions,
    has_perm = has_perm,
    has_perm_str = has_perm_str,
    check_patch_changes = check_patch_changes,
    check_patch_changes_str = check_patch_changes_str,
    cpce_to_string = cpce_to_string,
    test = test
}