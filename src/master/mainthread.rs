use khronos_ext::mlua_scheduler_ext::taskmgr::SchedulerImpl;
use khronos_runtime::rt::{KhronosRuntime, RuntimeCreateOpts};
use khronos_runtime::rt::mluau::prelude::*;

use crate::worker::limits::{MAX_VM_THREAD_STACK_SIZE, TEMPLATE_GIVE_TIME};

pub const MAX_MAIN_THREAD_STACK_SIZE: usize = 1024 * 1024 * 20; // 20MB maximum memory

#[allow(async_fn_in_trait)]
pub trait RunInThreadFn<Data, Resp> 
where Data: Send + 'static, 
Resp: Send + 'static 
{
    async fn run(rt: &KhronosRuntime, data: Data) -> Resp;
}

/// Helper method to run a function in a new thread with a KhronosRuntime, used for shell, command registration etc.
pub fn run_in_thread<R, RD, RR, FS>(vfs: FS, data: RD) -> RR
where R: RunInThreadFn<RD, RR> + 'static,
    RD: Send + 'static,
    RR: Send + 'static,
    FS: khronos_runtime::mluau_require::vfs::FileSystem + 'static
{
    let (tx, rx) = std::sync::mpsc::channel();

    std::thread::Builder::new()
        .stack_size(MAX_VM_THREAD_STACK_SIZE) // Increase stack size for the thread
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build_local(tokio::runtime::LocalOptions::default())
                .expect("Failed to create tokio runtime");
            rt.block_on(async move {
                let rt = KhronosRuntime::new(
                    RuntimeCreateOpts {
                        disable_task_lib: false,
                        time_limit: None,
                        give_time: TEMPLATE_GIVE_TIME
                    },
                    None::<(fn(&Lua, LuaThread) -> Result<(), LuaError>, fn(LuaLightUserData) -> ())>,
                    vfs,
                    "antiraid"
                )
                .expect("Failed to create KhronosRuntime");

                let resp = R::run(&rt, data).await;
                let _ = tx.send(resp);

                rt.scheduler().stop();            
            });
        })
        .expect("Failed to spawn thread")
        .join()
        .expect("Failed to join thread");

    rx.recv().expect("Failed to receive response from thread")
}