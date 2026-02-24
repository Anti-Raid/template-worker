mod globalcommand;

use std::sync::Arc;
use khronos_ext::mlua_scheduler_ext::{LuaSchedulerAsyncUserData, taskmgr::SchedulerImpl};
use khronos_runtime::rt::{KhronosRuntime, RuntimeCreateOpts, mluau::prelude::*};
use crate::{fauxpas::god::LuaKvGod, mesophyll::dbstate::KeyValueDb, worker::{builtins::TemplatingTypes, limits::{MAX_VM_THREAD_STACK_SIZE, TEMPLATE_GIVE_TIME}}};
use rust_embed::Embed;

#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/twshell"]
#[prefix = ""]
pub struct TwShell;

#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/twshell/_luaurcvfs"]
#[prefix = ""]
pub struct LuaurcVfs;


#[allow(dead_code)]
pub struct ShellData {
    pub pg_pool: sqlx::PgPool,
    pub http: Arc<serenity::http::Http>,
    pub reqwest: reqwest::Client,
    pub sandwich: crate::sandwich::Sandwich,
}

pub struct ShellContext {
    pub data: ShellData,
}

impl std::ops::Deref for ShellContext {
    type Target = ShellData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl LuaUserData for ShellContext {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        // Creates a new KvGod for interacting with the key-value store
        methods.add_method("CreateKvGod", |_lua, this, (): ()| {
            let kv_db = KeyValueDb::new(this.pg_pool.clone());
            let kv_god = LuaKvGod::new(kv_db);
            Ok(kv_god)
        });

        methods.add_scheduler_async_method("RegisterGlobalCommands", async move |lua, this, value: LuaValue| {
            let res: Vec<globalcommand::CreateCommand> = lua.from_value(value)
                .map_err(|e| LuaError::external(format!("Failed to parse command definitions: {e:?}")))?;

            let resp = this.http.create_global_commands(&res)
                .await
                .map_err(|e| LuaError::external(format!("Failed to register global commands: {e:?}")))?;

            lua.to_value(&resp)
        });

        methods.add_scheduler_async_method("GetInput", async move |_lua, _this, (prompt,): (String,)| {
            let (tx, rx) = tokio::sync::oneshot::channel();

            std::thread::spawn(move || {
                let mut editor = match rustyline::DefaultEditor::new() {
                    Ok(e) => e,
                    Err(e) => {
                        let _ = tx.send(Err(format!("Failed to create editor: {e}")));
                        return;
                    }
                };

                let input = match editor.readline(&prompt) {
                    Ok(i) => i,
                    Err(e) => {
                        let _ = tx.send(Err(format!("Failed to read input: {e}")));
                        return;
                    }
                };

                let _ = tx.send(Ok(input));
            });

            match rx.await {
                Ok(Ok(input)) => Ok(input),
                Ok(Err(e)) => Err(LuaError::external(e)),
                Err(_) => Err(LuaError::external("Failed to receive input")),
            }
        });
    }
}

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

pub fn init_shell(ctx: ShellData) {
    pub struct RunInThreadShell;
    impl RunInThreadFn<ShellData, ()> for RunInThreadShell {
        async fn run(rt: &KhronosRuntime, data: ShellData) -> () {
            let func = rt
            .eval_script::<LuaFunction>(
                "./shell",
            )
            .expect("Failed to import shell");

            let s_ctx = ShellContext { data };

            let ud = rt.call_in_scheduler::<_, LuaMultiValue>(func, s_ctx).await.expect("Failed to call shell main function");
            println!("Shell main function returned: {:?}", ud);
        }
    }

    run_in_thread::<RunInThreadShell, _, _, _>(
    vfs::OverlayFS::new(&vec![
            vfs::EmbeddedFS::<LuaurcVfs>::new().into(),
            vfs::EmbeddedFS::<TwShell>::new().into(),
            vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
        ]),
        ctx
    );
}
