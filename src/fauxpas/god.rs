use khronos_runtime::rt::{mlua::prelude::*, mlua_scheduler::LuaSchedulerAsyncUserData};
use crate::{fauxpas::base::LuaId, mesophyll::server::DbState, worker::workerlike::WorkerLike};

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
        methods.add_scheduler_async_method("settenantbanstate", async move |_lua, this, (id, banned): (LuaId, bool)| {
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
        methods.add_scheduler_async_method("setshoplistingreviewstate", async move |_lua, this, (listing_id, review_state): (String, String)| {
            this.ensure_secure()?;
            let listing_id = listing_id.parse::<uuid::Uuid>()
                .map_err(|e| LuaError::external(format!("Invalid listing ID: {e:?}")))?;
            sqlx::query("UPDATE shop_listings_v2 SET review_state = $1 WHERE listing_id = $2")
                .bind(review_state)
                .bind(listing_id)
                .execute(this.mesophyll_db_state.get_pool())
                .await
                .map_err(|e| LuaError::external(format!("Failed to set tenant ban state: {e:?}")))?;
            
            Ok(())
        });
    }
}