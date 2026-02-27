use khronos_runtime::rt::{mlua::prelude::*, mlua_scheduler::LuaSchedulerAsyncUserData};
use khronos_runtime::plugins::antiraid::kv::KvRecord;
use khronos_runtime::utils::khronos_value::KhronosValue;
use crate::geese::kv::SerdeKvRecord;
use crate::geese::kv::KeyValueDb;
use crate::fauxpas::base::LuaId;

pub struct LuaKvGod {
    kv_db: KeyValueDb
}

impl LuaKvGod {
    pub fn new(kv_db: KeyValueDb) -> Self {
        Self { kv_db }
    }

    pub fn cast_record(record: SerdeKvRecord) -> KvRecord {
        KvRecord {
            id: record.id,
            key: record.key,
            value: record.value,
            scopes: record.scopes,
            exists: true,
            created_at: record.created_at,
            last_updated_at: record.last_updated_at,
        }
    }
}

impl LuaUserData for LuaKvGod {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // Get a value from the KV store
        methods.add_scheduler_async_method("Get", async move |_lua, this, (id, scopes, key): (LuaId, Vec<String>, String)| {
            let res = this.kv_db.kv_get(id.0, scopes, key).await
                .map_err(|e| LuaError::external(format!("Failed to get KV value: {e:?}")))?;
            Ok(res.map(Self::cast_record))
        });

        // List scopes for a tenant in the KV store
        methods.add_scheduler_async_method("ListScopes", async move |_lua, this, id: LuaId| {
            let res = this.kv_db.kv_list_scopes(id.0).await
                .map_err(|e| LuaError::external(format!("Failed to list KV scopes: {e:?}")))?;
            Ok(res)
        });

        // Set a value in the KV store
        methods.add_scheduler_async_method("Set", async move |_lua, this, (id, scopes, key, value): (LuaId, Vec<String>, String, KhronosValue)| {
            let res = this.kv_db.kv_set(id.0, scopes, key, value).await
                .map_err(|e| LuaError::external(format!("Failed to set KV value: {e:?}")))?;
            Ok(res)
        });

        // Delete a value from the KV store
        methods.add_scheduler_async_method("Delete", async move |_lua, this, (id, scopes, key): (LuaId, Vec<String>, String)| {
            let res = this.kv_db.kv_delete(id.0, scopes, key).await
                .map_err(|e| LuaError::external(format!("Failed to delete KV value: {e:?}")))?;
            Ok(res)
        });

        // Find values in the KV store
        methods.add_scheduler_async_method("Find", async move |_lua, this, (id, scopes, prefix): (LuaId, Vec<String>, String)| {
            let res = this.kv_db.kv_find(id.0, scopes, prefix).await
                .map_err(|e| LuaError::external(format!("Failed to find KV values: {e:?}")))?;
            Ok(res.into_iter().map(Self::cast_record).collect::<Vec<_>>())
        });
    }

    fn register(registry: &mut LuaUserDataRegistry<Self>) {
        Self::add_fields(registry);
        Self::add_methods(registry);
        let fields = registry.fields(false).iter().map(|x| x.to_string()).collect::<Vec<_>>();
        registry.add_meta_field("__ud_fields", fields);
    }
}