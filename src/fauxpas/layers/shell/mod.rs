use std::sync::Arc;
use dapi::types::CreateCommand;
use khronos_ext::mlua_scheduler_ext::LuaSchedulerAsyncUserData;
use khronos_runtime::rt::{KhronosRuntime, mluau::prelude::*};
use tokio::sync::{oneshot::Sender as OneshotSender, mpsc::{unbounded_channel, UnboundedSender}};
use crate::worker::{builtins::TemplatingTypes};
use crate::fauxpas::geese::LuaKvGod;
use crate::geese::{sandwich::Sandwich, kv::KeyValueDb};
use rust_embed::Embed;
use crate::fauxpas::mainthread::{run_in_thread, RunInThreadFn};

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
    pub sandwich: Sandwich,
}

type ShellInputValue = Result<Option<String>, String>;

pub struct ShellContext {
    pub data: ShellData,
    pub input_handle: UnboundedSender<(String, OneshotSender<ShellInputValue>)>,
    pub _jh: std::thread::JoinHandle<()>, // Ensure input thread is dropped when ShellContext is dropped
}

impl ShellContext {
    pub fn new(data: ShellData) -> Self {
        let (tx, mut rx) = unbounded_channel::<(String, OneshotSender<ShellInputValue>)>();
        let jh = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create Tokio runtime for shell input");

            rt.block_on(async move {
                let mut editor = match rustyline::DefaultEditor::new() {
                    Ok(e) => e,
                    Err(e) => {
                        log::error!("Failed to create editor: {e}");
                        return;
                    }
                };

                while let Some((prompt, responder)) = rx.recv().await {
                    match editor.readline(&prompt) {
                        Ok(i) => {
                            let _ = editor.add_history_entry(&i);
                            let _ = responder.send(Ok(Some(i)));
                        },
                        Err(e) => {
                            match e {
                                rustyline::error::ReadlineError::Interrupted | rustyline::error::ReadlineError::Eof => {
                                    let _ = responder.send(Ok(None));
                                }
                                _ => {
                                    let _ = responder.send(Err(format!("Error reading line: {e}")));
                                }
                            }
                        }
                    };
                }
            });
        });
        Self { data, input_handle: tx, _jh: jh }
    }
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
            let res: Vec<CreateCommand> = lua.from_value(value)
                .map_err(|e| LuaError::external(format!("Failed to parse command definitions: {e:?}")))?;

            let resp = this.http.create_global_commands(&res)
                .await
                .map_err(|e| LuaError::external(format!("Failed to register global commands: {e:?}")))?;

            lua.to_value(&resp)
        });

        methods.add_scheduler_async_method("GetInput", async move |_lua, this, (prompt,): (String,)| {
            let (tx, rx) = tokio::sync::oneshot::channel();
            this.input_handle.send((prompt, tx))
                .map_err(|e| LuaError::external(format!("Failed to send input request: {e}")))?;

            match rx.await {
                Ok(Ok(input)) => Ok(input),
                Ok(Err(e)) => Err(LuaError::external(format!("Error getting input: {e}"))),
                Err(e) => Err(LuaError::external(format!("Input request was cancelled: {e}"))),
            }
        });

        methods.add_method("Log", |_lua, _this, values: LuaMultiValue| {
            khronos_runtime::utils::pp::pretty_print(values);
            Ok(())
        });
    }
}

pub fn init_shell(ctx: ShellData) {
    pub struct RunInThreadShell;
    impl RunInThreadFn<ShellData, ()> for RunInThreadShell {
        async fn run(rt: &KhronosRuntime, data: ShellData) -> () {
            let func = rt
            .eval_script::<LuaFunction>(
                "./entrypoint.shell",
            )
            .expect("Failed to import shell");

            let s_ctx = ShellContext::new(data);

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
