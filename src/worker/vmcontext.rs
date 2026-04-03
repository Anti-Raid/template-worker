use super::workervmmanager::Id;
use crate::worker::syscall::{SyscallArgs, SyscallHandler, SyscallRet};
use crate::worker::workertenantstate::WorkerTenantState;
use crate::worker::workervmmanager::VmData;
use khronos_runtime::traits::context::KhronosContext;

#[derive(Clone)]
pub struct TemplateContextProvider {
    /// system call handler
    syscall_handler: SyscallHandler,

    id: Id,
}

impl TemplateContextProvider {
    /// Creates a new `TemplateContextProvider` with the given template data
    pub fn new(id: Id, vm_data: VmData, wts: WorkerTenantState) -> Self {
        Self { id, syscall_handler: SyscallHandler::new(vm_data.state, wts, vm_data.kv_constraints, vm_data.ratelimits) }
    }
}

impl KhronosContext for TemplateContextProvider {
    type SyscallArgs = SyscallArgs;
    type SyscallRet = SyscallRet;


    async fn syscall(&self, args: SyscallArgs) -> Result<SyscallRet, khronos_runtime::Error> {
        self.syscall_handler.handle_syscall(self.id, args).await
    }
}
