local context = require"@self/context"
local setup = require"@self/dispatch"
local paginate = require"@self/paginate"

return {
    context = context,
    setup = setup,
    paginate = paginate.paginate
}