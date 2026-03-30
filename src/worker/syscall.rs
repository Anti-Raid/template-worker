use crate::{geese::state::{StateExecResult, StateOp}, worker::{workerstate::WorkerState, workertenantstate::WorkerTenantState, workervmmanager::Id}};
use khronos_runtime::rt::mluau::prelude::*;

/// The core underlying syscall
pub enum SyscallArgs {
    State {
        // Set of state ops to perform, all ops here are guaranteed to be atomically handled
        ops: Vec<StateOp>
    }
}

impl FromLua for SyscallArgs {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "SyscallArgs".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: String = tab.get("op")?;
        match typ.as_str() {
            "State" => {
                let ops = tab.get("ops")?;
                Ok(Self::State { ops })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "SyscallArgs".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

pub enum SyscallRet {
    State {
        res: Vec<StateExecResult>
    }
}

impl IntoLua for SyscallRet {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        match self {
            Self::State { res } => {
                table.set("res", res)?;
            }
        }
        table.set_readonly(true);
        Ok(LuaValue::Table(table))
    }
}

/// A syscall handler enables VMs to perform syscalls with the host to access certain host-defined functions
#[derive(Clone)]
pub struct SyscallHandler {
    state: WorkerState,
    wts: WorkerTenantState,
}

impl SyscallHandler {
    /// Creates a new syscall handler
    pub fn new(state: WorkerState, wts: WorkerTenantState) -> Self {
        Self { state, wts }
    }

    /// Handles a syscall
    pub async fn handle_syscall(&self, id: Id, args: SyscallArgs) -> Result<SyscallRet, crate::Error> {
        match args {
            SyscallArgs::State { ops } => {
                let res = self.state.mesophyll_client.exec_state_op(id, ops).await?;
                if let Some((ts_new_events, ts_new_flags)) = res.new_tenant_state {
                    self.wts.reload_for_tenant(id, ts_new_events, ts_new_flags, None).map_err(|e| e.to_string())?;
                }

                Ok(SyscallRet::State { res: res.results })
            }
        }
    }
}