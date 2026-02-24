use std::thread::JoinHandle;
use khronos_ext::mlua_scheduler_ext::LuaSchedulerAsyncUserData;
use khronos_runtime::{rt::mluau::prelude::*, utils::khronos_value::KhronosValue};
use serenity::async_trait;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tokio::sync::oneshot::{Sender as OneshotSender, Receiver as OneshotReciever};

pub const MAX_MAIN_THREAD_STACK_SIZE: usize = 1024 * 1024 * 20; // 20MB maximum memory

#[allow(dead_code)]
/// A LuaUserData wrapper around a WorkerLike implementation for use in Luau staff APIs
enum MainThreadMessage {
    AddTask((Box<dyn Taskable>, OneshotSender<KhronosValue>)),
    Shutdown,
}

/// The 'main thread' is the special multithreaded tokio thread containing all core tasks
#[allow(dead_code)]
pub struct MainThread {
    tx: UnboundedSender<MainThreadMessage>,
    _handle: JoinHandle<()>,
}

#[allow(dead_code)]
impl MainThread {
    pub fn new() -> Self {
        let (tx, mut rx) = unbounded_channel::<MainThreadMessage>();

        let th = std::thread::Builder::new()
        .name("mainthread".to_string())
        .stack_size(MAX_MAIN_THREAD_STACK_SIZE) // Increase stack size for the thread
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime");

            rt.block_on(async move {
                while let Some(message) = rx.recv().await {
                    match message {
                        MainThreadMessage::AddTask((task, responder)) => {
                            // Spawn the task and send the result back through the responder
                            tokio::task::spawn(async move {
                                let resp = task.exec().await;
                                let _ = responder.send(resp);
                            });
                        }
                        MainThreadMessage::Shutdown => {
                            break; // Exit the loop to shut down the main thread
                        }
                    }
                }
            });
        })
        .expect("Failed to spawn main thread");
        Self {
            tx,
            _handle: th,
        }
    }
}

pub struct KhronosValueRx(Option<OneshotReciever<KhronosValue>>);
impl LuaUserData for KhronosValueRx {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_scheduler_async_method_mut("Recv", async move |_lua, mut this, _: ()| {
            let rx = this.0.take().ok_or_else(|| LuaError::external("Attempted to receive from a KhronosValueRx more than once"))?;
            let res = rx.await.map_err(|e| LuaError::external(format!("Failed to receive KhronosValue: {e:?}")))?;
            Ok(res)
        });
    }
}

impl LuaUserData for MainThread {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_scheduler_async_method("AddTask", async move |_, this, task: LuaUserDataRef<Task>| {
            let task = task.0.clone_to_box();
            let (responder_tx, responder_rx) = tokio::sync::oneshot::channel();
            this.tx.send(MainThreadMessage::AddTask((task, responder_tx)))
                .map_err(|e| LuaError::external(format!("Failed to send task to main thread: {e:?}")))?;
            Ok(KhronosValueRx(Some(responder_rx)))
        });
    }
}

#[async_trait]
pub trait Taskable: Send + Sync {
    fn clone_to_box(&self) -> Box<dyn Taskable>;
    async fn exec(&self) -> KhronosValue;
}

/// A task that should be started on the main tokio runtime
pub struct Task(pub Box<dyn Taskable>);
impl LuaUserData for Task {}