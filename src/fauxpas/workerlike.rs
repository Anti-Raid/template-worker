use crate::{fauxpas::base::LuaId, worker::{workerlike::WorkerLike, workerpool::WorkerPool}};
use khronos_runtime::{primitives::event::CreateEvent, rt::{mlua_scheduler::LuaSchedulerAsyncUserData, mluau::prelude::*}};

#[allow(dead_code)]
/// A LuaWorkerLike wraps a WorkerLike implementation for use in Luau staff APIs
pub struct LuaWorkerLike<T: WorkerLike> {
    wl: T,
}

#[allow(dead_code)]
impl<T: WorkerLike> LuaWorkerLike<T> {
    pub fn new(wl: T) -> Self {
        Self { wl }
    }
}

/*
    /// Returns the worker's ID, if present
    /// 
    /// May return 0 for worker pools etc where a worker ID is not applicable
    fn id(&self) -> usize {
        0
    }

    fn clone_to_arc(&self) -> Arc<dyn WorkerLike + Send + Sync>;

    /// Runs a script with the given chunk name, code and event
    /// 
    /// This is the special version of dispatch event that directly enables for running arbitrary scripts
    /// (which is useful for the fauxpas staff API and other future internal tooling etc.)
    async fn run_script(&self, id: Id, name: String, code: String, event: CreateEvent) -> Result<KhronosValue, crate::Error>;

    /// Kill the worker like
    async fn kill(&self) -> Result<(), crate::Error>;

    /// Dispatch an event to the templates managed by this worker
    async fn dispatch_event(&self, id: Id, event: CreateEvent) -> Result<KhronosValue, crate::Error>;

    /// Dispatch an event to the templates managed by this worker without waiting for the result
    fn dispatch_event_nowait(&self, id: Id, event: CreateEvent) -> Result<(), crate::Error>;

    /// Drop a tenant from the worker
    async fn drop_tenant(&self, id: Id) -> Result<(), crate::Error>;

    /// For a pool, returns the length of the pool
    /// 
    /// Returns 0 for non-pool workers
    fn len(&self) -> usize {
        0
    }
 */

#[allow(dead_code)]
impl<T: WorkerLike> LuaUserData for LuaWorkerLike<T> {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("ID", |_, this, ()| {
            Ok(this.wl.id())
        });

        methods.add_scheduler_async_method("Kill", async move |_, this, _: ()| {
            this.wl.kill().await.map_err(|x| LuaError::external(x.to_string()))?;
            Ok(())
        });

        methods.add_scheduler_async_method("RunScript", async move |_lua, this, (id, name, code, event): (LuaId, String, String, CreateEvent)| {
            let res = this.wl.run_script(id.0, name, code, event).await
                .map_err(|x| LuaError::external(x.to_string()))?;
            Ok(res)
        });

        methods.add_scheduler_async_method("DispatchEvent", async move |_lua, this, (id, event): (LuaId, CreateEvent)| {
            let res = this.wl.dispatch_event(id.0, event).await
                .map_err(|x| LuaError::external(x.to_string()))?;
            Ok(res)
        });

        methods.add_method("DispatchEventNoWait", |_lua, this, (id, event): (LuaId, CreateEvent)| {
            this.wl.dispatch_event_nowait(id.0, event)
                .map_err(|x| LuaError::external(x.to_string()))?;
            Ok(())
        });

        methods.add_scheduler_async_method("DropTenant", async move |_, this, id: LuaId| {
            this.wl.drop_tenant(id.0).await
                .map_err(|x| LuaError::external(x.to_string()))?;
            Ok(())
        });

        methods.add_meta_method(LuaMetaMethod::Len, |_, this, _: ()| {
            Ok(this.wl.len())
        });
    }
}

#[allow(dead_code)]
/// A LuaWorkerPool wraps a WorkerPool for use in Luau staff APIs
pub struct LuaWorkerPool<T: WorkerLike> {
    wp: WorkerPool<T>,
}

#[allow(dead_code)]
impl<T: WorkerLike> LuaWorkerPool<T> {
    pub fn new(wp: WorkerPool<T>) -> Self {
        Self { wp }
    }
}

impl<T: WorkerLike> LuaUserData for LuaWorkerPool<T> {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // Gets the WorkerLike for a given tenant ID
        methods.add_method("GetWorkerFor", |_lua, this, id: LuaId| {
            let worker = this.wp.get_worker_for(id.0);
            let lua_workerlike = LuaWorkerLike::new(worker.clone_to_arc());
            Ok(lua_workerlike)
        });

        // Casts the WorkerPool to a WorkerLike
        methods.add_method("AsWorkerLike", |_lua, this, ()| {
            let lua_workerlike = LuaWorkerLike::new(this.wp.clone_to_arc());
            Ok(lua_workerlike)
        });

        // Returns the number of workers in the pool
        methods.add_meta_method(LuaMetaMethod::Len, |_, this, _: ()| {
            Ok(this.wp.len())
        });
    }
}