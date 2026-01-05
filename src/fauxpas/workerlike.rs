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

#[allow(dead_code)]
impl<T: WorkerLike> LuaUserData for LuaWorkerLike<T> {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("id", |_, this, ()| {
            Ok(this.wl.id())
        });

        methods.add_scheduler_async_method("kill", async move |_, this, _: ()| {
            this.wl.kill().await.map_err(|x| LuaError::external(x.to_string()))?;
            Ok(())
        });

        methods.add_scheduler_async_method("runscript", async move |lua, this, (id, name, code, event): (LuaId, String, String, LuaValue)| {
            let event: CreateEvent = lua.from_value(event)?;
            let res = this.wl.run_script(id.0, name, code, event).await
                .map_err(|x| LuaError::external(x.to_string()))?;
            Ok(res)
        });

        methods.add_scheduler_async_method("dispatchevent", async move |lua, this, (id, event): (LuaId, LuaValue)| {
            let event: CreateEvent = lua.from_value(event)?;
            let res = this.wl.dispatch_event(id.0, event).await
                .map_err(|x| LuaError::external(x.to_string()))?;
            Ok(res)
        });

        methods.add_method("dispatcheventnowait", |lua, this, (id, event): (LuaId, LuaValue)| {
            let event: CreateEvent = lua.from_value(event)?;
            this.wl.dispatch_event_nowait(id.0, event)
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
        methods.add_method("getworkerfor", |_lua, this, id: LuaId| {
            let worker = this.wp.get_worker_for(id.0);
            let lua_workerlike = LuaWorkerLike::new(worker.clone_to_arc());
            Ok(lua_workerlike)
        });

        // Casts the WorkerPool to a WorkerLike
        methods.add_method("asworkerlike", |_lua, this, ()| {
            let lua_workerlike = LuaWorkerLike::new(this.wp.clone_to_arc());
            Ok(lua_workerlike)
        });

        // Returns the number of workers in the pool
        methods.add_meta_method(LuaMetaMethod::Len, |_, this, _: ()| {
            Ok(this.wp.len())
        });
    }
}