
--- A lazy value is a value that is only serialized when accessed
export type Lazy<T> = {
    --- The inner value. Only serialized when accessed and then cached.
    read data: T,
}

return {}