use khronos_runtime::rt::{mlua::prelude::*, mlua_scheduler::LuaSchedulerAsyncUserData};
use khronos_runtime::plugins::antiraid::kv::KvRecord;
use khronos_runtime::utils::khronos_value::KhronosValue;
use crate::mesophyll::dbtypes::SerdeKvRecord;
use crate::{fauxpas::base::LuaId, mesophyll::{self, dbstate::DbState}, worker::workerlike::WorkerLike};

/// LuaGod is a special God-level entity that provides access to everything else in the staff lua api
/// 
/// Must only exist on the master's 'secure fauxpas VM'
pub struct LuaGod<T: WorkerLike> {
    mesophyll_db_state: DbState,
    wl: T,
    secure: bool,
}

#[allow(dead_code)]
impl<T: WorkerLike> LuaGod<T> {
    /// Creates a new LuaGod instance
    pub fn new(mesophyll_db_state: DbState, wl: T, secure: bool) -> Self {
        Self { mesophyll_db_state, secure, wl }
    }

    /// Returns an error if this LuaGod is not secure
    pub fn ensure_secure(&self) -> LuaResult<()> {
        if !self.secure {
            return Err(LuaError::runtime("Attempted to call a secure-only method on an insecure LuaGod"));
        }
        Ok(())
    }
}

impl<T: WorkerLike> LuaUserData for LuaGod<T> {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // Set tenant ban state
        methods.add_scheduler_async_method("SetTenantBanState", async move |_lua, this, (id, banned): (LuaId, bool)| {
            this.ensure_secure()?;
            sqlx::query("UPDATE tenant_state SET banned = $1 WHERE owner_id = $2 AND owner_type = $3")
                .bind(banned)
                .bind(id.0.tenant_id())
                .bind(id.0.tenant_type())
                .execute(this.mesophyll_db_state.get_pool())
                .await
                .map_err(|e| LuaError::external(format!("Failed to set tenant ban state: {e:?}")))?;
            
            this.wl.drop_tenant(id.0).await
            .map_err(|e| LuaError::external(format!("Failed to drop tenant after ban state change: {e:?}")))?;

            Ok(())
        });

        // Set shop listing review state
        methods.add_scheduler_async_method("SetGlobalKvReviewState", async move |_lua, this, (key, version, review_state): (String, i32, String)| {
            this.ensure_secure()?;
            sqlx::query("UPDATE global_kv SET review_state = $1 WHERE key = $2 AND version = $3")
                .bind(review_state)
                .bind(key)
                .bind(version)
                .execute(this.mesophyll_db_state.get_pool())
                .await
                .map_err(|e| LuaError::external(format!("Failed to set tenant ban state: {e:?}")))?;
            
            Ok(())
        });

        // Create a db client
        methods.add_method("CreateDbClient", |_lua, this, (): ()| {
            this.ensure_secure()?;
            let pool = this.mesophyll_db_state.get_pool().clone();

            let db_client = super::db::Db::new(pool);
            Ok(db_client)
        });

        // Create a KV god
        methods.add_method("CreateKvGod", |_lua, this, (): ()| {
            this.ensure_secure()?;
            let kv_god = LuaKvGod::new(this.mesophyll_db_state.key_value_db().clone());
            Ok(kv_god)
        });
    }

    fn register(registry: &mut LuaUserDataRegistry<Self>) {
        Self::add_fields(registry);
        Self::add_methods(registry);
        let fields = registry.fields(false).iter().map(|x| x.to_string()).collect::<Vec<_>>();
        registry.add_meta_field("__ud_fields", fields);
    }
}

pub struct LuaKvGod {
    kv_db: mesophyll::dbstate::KeyValueDb
}

impl LuaKvGod {
    pub fn new(kv_db: mesophyll::dbstate::KeyValueDb) -> Self {
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