use khronos_ext::mlua_scheduler_ext::LuaSchedulerAsyncUserData;
use khronos_runtime::rt::{KhronosRuntime, mluau::prelude::*};
use tokio::sync::{oneshot::Sender as OneshotSender, mpsc::{unbounded_channel, UnboundedSender}};
use crate::{master::syscall::MSyscallArgs, mesophyll::client::MesophyllShellClient, worker::builtins::TemplatingTypes};
use rust_embed::Embed;
use crate::master::mainthread::{run_in_thread, RunInThreadFn};

#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/twshell"]
#[prefix = ""]
pub struct TwShell;

#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/twshell/_luaurcvfs"]
#[prefix = ""]
pub struct LuaurcVfs;


type ShellInputValue = Result<Option<String>, String>;

pub struct ShellContext {
    pub shell_meso: MesophyllShellClient,
    pub input_handle: UnboundedSender<(String, OneshotSender<ShellInputValue>)>,
    pub _jh: std::thread::JoinHandle<()>, // Ensure input thread is dropped when ShellContext is dropped
}

impl ShellContext {
    pub fn new(shell_meso: MesophyllShellClient) -> Self {
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
        Self { shell_meso, input_handle: tx, _jh: jh }
    }
}

impl LuaUserData for ShellContext {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_scheduler_async_method("MSyscall", async move |lua, this, args: MSyscallArgs| {
            let resp = this.shell_meso.msyscall(args).await;
            let table = lua.create_table_with_capacity(0, 2)?;
            match resp {
                Ok(r) => {
                    table.set("status", "Ok")?;
                    table.set("data", r)?;
                }
                Err(r) => {
                    table.set("status", "Err")?;
                    table.set("data", r)?;
                }
            }
            Ok(table)
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

pub fn init_shell(shell_meso: MesophyllShellClient) {
    pub struct RunInThreadShell;
    impl RunInThreadFn<MesophyllShellClient, ()> for RunInThreadShell {
        async fn run(rt: &KhronosRuntime, data: MesophyllShellClient) -> () {
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
        shell_meso
    );
}
