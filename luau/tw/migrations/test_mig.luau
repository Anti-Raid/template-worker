local db = require"../common/db"

return {
    up = function(db: db.Db)
        print("Running test migration")
    end,
}  