use super::workerstate::WorkerState;
use super::workervmmanager::Id;
use crate::worker::builtins::EXPOSED_VFS;
use crate::worker::syscall::{SyscallArgs, SyscallHandler, SyscallRet};
use crate::worker::workertenantstate::WorkerTenantState;
use crate::worker::workervmmanager::VmData;
use khronos_runtime::core::typesext::Vfs;
use khronos_runtime::traits::context::KhronosContext;
use khronos_runtime::traits::ir::runtime as runtime_ir;
use khronos_runtime::traits::runtimeprovider::RuntimeProvider;
use std::sync::Arc;
use super::limits::Ratelimits;

#[derive(Clone)]
pub struct TemplateContextProvider {
    state: WorkerState,

    /// system call handler
    syscall_handler: SyscallHandler,

    id: Id,
    
    /// The ratelimits of the VM
    ratelimits: Arc<Ratelimits>,
}

impl TemplateContextProvider {
    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(
        id: Id,
        vm_data: VmData,
        wts: WorkerTenantState
    ) -> Self {
        Self {
            id,
            syscall_handler: SyscallHandler::new(vm_data.state.clone(), wts, vm_data.kv_constraints, vm_data.ratelimits.clone()),
            state: vm_data.state,
            ratelimits: vm_data.ratelimits,
        }
    }

    fn id(&self) -> Id {
        self.id.clone()
    }
}

impl KhronosContext for TemplateContextProvider {
    type RuntimeProvider = ArRuntimeProvider;

    fn runtime_provider(&self) -> Option<Self::RuntimeProvider> {
        Some(ArRuntimeProvider {
            id: self.id(),
            state: self.state.clone(),
            syscall_handler: self.syscall_handler.clone(),
            ratelimits: self.ratelimits.clone(),
        })
    }
}

#[derive(Clone)]
pub struct ArRuntimeProvider {
    id: Id,
    state: WorkerState,
    syscall_handler: SyscallHandler,
    ratelimits: Arc<Ratelimits>,
}

impl RuntimeProvider for ArRuntimeProvider {
    type SyscallArgs = SyscallArgs;
    type SyscallRet = SyscallRet;

    fn attempt_action(&self, bucket: &str) -> Result<(), khronos_runtime::Error> {
        self.ratelimits.runtime.check(bucket)
    }

    fn get_exposed_vfs(&self) -> Result<std::collections::HashMap<String, Vfs>, khronos_runtime::Error> {
        Ok((&*EXPOSED_VFS).clone())
    }

    async fn stats(&self) -> Result<runtime_ir::RuntimeStats, khronos_runtime::Error> {
        log::info!("Fetching runtime stats for tenant {:?}", self.id);
        let resp = self.state.stratum.get_status().await?;

        Ok(runtime_ir::RuntimeStats {
            total_cached_guilds: resp.guild_count, // This field is deprecated, use total_guilds instead
            total_guilds: resp.guild_count,
            total_users: resp.user_count,
            //total_members: sandwich_resp.total_members.try_into()?,
            last_started_at: crate::CONFIG.start_time,
        })
    }

    fn event_list(&self) -> Result<Vec<String>, khronos_runtime::Error> {
        let mut vec = dapi::EVENT_LIST
            .iter()
            .copied()
            .map(|x| x.to_string())
            .collect::<Vec<String>>();


        vec.push("OnStartup".to_string());
        vec.push("KeyExpiry".to_string());

        Ok(vec)
    }

    fn links(&self) -> Result<runtime_ir::RuntimeLinks, khronos_runtime::Error> {
        let support_server = crate::CONFIG.meta.support_server_invite.clone();
        let api_url = crate::CONFIG.sites.api.clone();
        let frontend_url = crate::CONFIG.sites.frontend.clone();
        let docs_url = crate::CONFIG.sites.docs.clone();

        Ok(runtime_ir::RuntimeLinks {
            support_server,
            api_url,
            frontend_url,
            docs_url,
        })
    }

    async fn syscall(&self, args: SyscallArgs) -> Result<SyscallRet, khronos_runtime::Error> {
        self.syscall_handler.handle_syscall(self.id, args).await
    }
}
