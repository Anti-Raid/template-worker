local datetime = require "@antiraid/datetime"
local typesext = require "@antiraid/typesext"   

export type KhronosValue = string -- Text
    | number -- Integer, Float
    | integer -- Int64
    | boolean -- Boolean
    | vector -- Vector
    | { [string]: KhronosValue } -- Map
    | { KhronosValue } -- List
    | datetime.DateTime -- Timestamptz
    | datetime.TimeDelta -- Interval
    | datetime.TimeZone -- TimeZone
    | typesext.MemoryVfs -- Memory VFS
    | nil -- Null

return {}