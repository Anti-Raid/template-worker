local datetime = require"@antiraid/datetime"
local typesext = require"@antiraid/typesext"
local db = require"../common/db"

local OLD_SCOPE = "builtins.stings"
local KEX_SCOPE = "__kexsbuiltins.stings"
local FAR_FUTURE_SECS = 100 * 365 * 24 * 60 * 60

return {
    up = function(db: db.Db)
        print("[stings] merging old sting data into kex records (pre-kv_scope_unnest)")

        local tx = db:begin()

        local ok, err = pcall(function()
            local sting_rows = tx:fetchall(
                "SELECT owner_id, owner_type, key, scopes, value FROM tenant_kv WHERE $1 = ANY(scopes)",
                { db:cast(OLD_SCOPE, "string") }
            )

            type StingFields = { owner_id: string, owner_type: string, key: string, userid: string, modid: string?, reason: string, stings: number }
            local old_stings: {[string]: StingFields} = {}
            local missing_userid = 0
            for _, row in sting_rows do
                local owner_id = row:get(0, "string"):take() :: string
                local owner_type = row:get(1, "string"):take() :: string
                local key = row:get(2, "string"):take() :: string
                local scopes = row:get(3, "{string}"):take() :: {string}
                local value = row:get(4, "custom@khronosvalue"):take() :: any

                local userid: string? = nil
                for _, s in scopes do
                    if s ~= OLD_SCOPE then
                        userid = s
                        break
                    end
                end

                if not userid then
                    print(string.format("[stings] sting key=%s owner=%s/%s has no userid in scopes %s; skipping", key, owner_type, owner_id, table.concat(scopes, ",")))
                    missing_userid += 1
                    continue
                end

                local composite = owner_id .. "|" .. owner_type .. "|" .. key
                old_stings[composite] = {
                    owner_id = owner_id,
                    owner_type = owner_type,
                    key = key,
                    userid = userid,
                    modid = value.modId or value.modid,
                    reason = value.reason or "",
                    stings = value.stings or 1,
                }
            end
            print(string.format("[stings] loaded %d old sting rows (%d skipped)", #sting_rows - missing_userid, missing_userid))

            local kex_rows = tx:fetchall(
                "SELECT id, owner_id, owner_type, key, value FROM tenant_kv WHERE $1 = ANY(scopes)",
                { db:cast(KEX_SCOPE, "string") }
            )
            print(string.format("[stings] found %d existing kex rows", #kex_rows))

            local merged = 0
            local matched: {[string]: boolean} = {}

            for _, row in kex_rows do
                local id = row:get(0, "string"):take() :: string
                local owner_id = row:get(1, "string"):take() :: string
                local owner_type = row:get(2, "string"):take() :: string
                local key = row:get(3, "string"):take() :: string
                local value = row:get(4, "custom@khronosvalue"):take() :: any

                local composite = owner_id .. "|" .. owner_type .. "|" .. key
                local sting = old_stings[composite]
                if not sting then
                    print(string.format("[stings] kex key=%s owner=%s/%s has no matching sting data; leaving as-is", key, owner_type, owner_id))
                    continue
                end
                matched[composite] = true

                local new_value = {
                    expiresat = value.expiresat,
                    data = {
                        userid = sting.userid,
                        modid = sting.modid,
                        reason = sting.reason,
                        stings = sting.stings,
                    },
                }

                tx:execute(
                    "UPDATE tenant_kv SET value = $1, last_updated_at = NOW() WHERE id = $2",
                    {
                        db:cast(new_value, "custom@khronosvalue"),
                        db:cast(id, "string"),
                    }
                )
                merged += 1
            end
            print(string.format("[stings] merged %d kex rows with matching sting data", merged))

            local inserted = 0
            local far_future = datetime.UTC:now() + datetime.timedelta_seconds(FAR_FUTURE_SECS)
            for composite, sting in old_stings do
                if matched[composite] then continue end

                local new_id = typesext.randstring(64)
                local new_value = {
                    expiresat = far_future,
                    data = {
                        userid = sting.userid,
                        modid = sting.modid,
                        reason = sting.reason,
                        stings = sting.stings,
                    },
                }

                tx:execute(
                    [[INSERT INTO tenant_kv (id, owner_id, owner_type, key, value, scopes)
                      VALUES ($1, $2, $3, $4, $5, $6)
                      ON CONFLICT (owner_id, owner_type, key, scopes) DO UPDATE SET value = EXCLUDED.value, last_updated_at = NOW()]],
                    {
                        db:cast(new_id, "string"),
                        db:cast(sting.owner_id, "string"),
                        db:cast(sting.owner_type, "string"),
                        db:cast(sting.key, "string"),
                        db:cast(new_value, "custom@khronosvalue"),
                        db:cast({KEX_SCOPE}, "{string}"),
                    }
                )
                inserted += 1
            end
            if inserted > 0 then
                print(string.format("[stings] inserted %d new kex rows (no prior expiry, assigned far-future)", inserted))
            end

            local deleted = tx:execute(
                "DELETE FROM tenant_kv WHERE $1 = ANY(scopes)",
                { db:cast(OLD_SCOPE, "string") }
            )
            print(string.format("[stings] deleted %d rows from scope='%s'", deleted, OLD_SCOPE))
        end)

        if not ok then
            print("[stings] migration failed: " .. tostring(err))
            tx:rollback()
            error(err)
        end

        tx:commit()
        print("[stings] committed")
    end,
}
