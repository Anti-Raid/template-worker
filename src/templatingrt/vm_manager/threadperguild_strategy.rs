use super::client::ArLua;
use super::client::{ArLuaHandle, LuaVmResult};
use crate::templatingrt::state::CreateGuildState;
use crate::templatingrt::vm_manager::client::LuaVmAction;
use crate::templatingrt::vm_manager::core::configure_runtime_manager;
use crate::templatingrt::MAX_VM_THREAD_STACK_SIZE;
use serenity::all::GuildId;
use std::rc::Rc;
use std::sync::atomic::{Ordering, AtomicUsize};

pub static NUM_THREADS: AtomicUsize = AtomicUsize::new(0);

#[allow(dead_code)]
pub async fn create_lua_vm(
    guild_id: GuildId,
    cgs: CreateGuildState
) -> Result<ArLua, silverpelt::Error> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    )>();

    std::thread::Builder::new()
        .name(format!("lua-vm-{}", guild_id))
        .stack_size(MAX_VM_THREAD_STACK_SIZE)
        .spawn(move || {
            NUM_THREADS.fetch_add(1, Ordering::SeqCst);
            super::perthreadpanichook::set_hook(Box::new(move |_| {
                NUM_THREADS.fetch_sub(1, Ordering::SeqCst);
                super::remove_vm(guild_id);
            }));

            let gs = Rc::new(
                cgs.to_guild_state(guild_id)
                    .expect("Failed to create Lua VM userdata"),
            );

            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime");

            let local = tokio::task::LocalSet::new();

            local.block_on(&rt, async {
                let tis_ref = match configure_runtime_manager() {
                    Ok(tis) => tis,
                    Err(e) => {
                        log::error!("Failed to configure Lua VM: {}", e);
                        panic!("Failed to configure Lua VM");
                    }
                };

                tis_ref.set_on_broken(Box::new(move || {
                    super::remove_vm(guild_id);
                }));

                while let Some((action, callback)) = rx.recv().await {
                    let tis_ref = tis_ref.clone();
                    let gs = gs.clone();

                    local.spawn_local(async move {
                        action.handle(tis_ref, gs, callback).await
                    });
                }
            });
        })?;

    Ok(ArLua::ThreadPerGuild(PerThreadLuaHandle { handle: tx }))
}

#[derive(Clone)]
pub struct PerThreadLuaHandle {
    #[allow(clippy::type_complexity)]
    /// The thread handle for the Lua VM
    pub handle: tokio::sync::mpsc::UnboundedSender<(
        LuaVmAction,
        tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    )>,
}

impl ArLuaHandle for PerThreadLuaHandle {
    fn send_action(
        &self,
        action: LuaVmAction,
        callback: tokio::sync::oneshot::Sender<Vec<(String, LuaVmResult)>>,
    ) -> Result<(), khronos_runtime::Error> {
        self.handle.send((action, callback))?;
        Ok(())
    }
}
