mod objstorage;
mod cdn;

use std::sync::Arc;

use crate::{geese::state::{StateExecResult, StateOp}, worker::{limits::{LuaKVConstraints, Ratelimits}, syscall::{cdn::{CdnCall, CdnResult}, objstorage::{ObjectStorageCall, ObjectStorageResult}}, workerstate::WorkerState, workertenantstate::WorkerTenantState, workervmmanager::Id}};
use khronos_runtime::rt::mluau::prelude::*;
use log::info;

/// The core underlying syscall
#[derive(Debug)]
pub enum SyscallArgs {
    State {
        // Set of state ops to perform, all ops here are guaranteed to be atomically handled
        ops: Vec<StateOp>
    },
    ObjectStorage {
        op: ObjectStorageCall
    },
    Cdn {
        op: CdnCall
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
            "ObjectStorage" => {
                let op = tab.get("req")?;
                Ok(Self::ObjectStorage { op })
            },
            "Cdn" => {
                let op = tab.get("req")?;
                Ok(Self::Cdn { op })
            }
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
    },
    ObjectStorage {
        res: ObjectStorageResult
    },
    Cdn {
        res: CdnResult
    }
}

impl IntoLua for SyscallRet {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 2)?;
        match self {
            Self::State { res } => {
                table.set("op", "State")?;
                table.set("res", res)?;
            }
            Self::ObjectStorage { res } => {
                table.set("op", "ObjectStorage")?;
                table.set("res", res)?;
            }
            Self::Cdn { res } => {
                table.set("op", "Cdn")?;
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
    kv_constraints: LuaKVConstraints,
    ratelimits: Arc<Ratelimits>,
}

impl SyscallHandler {
    /// Creates a new syscall handler
    pub fn new(state: WorkerState, wts: WorkerTenantState, kv_constraints: LuaKVConstraints, ratelimits: Arc<Ratelimits>) -> Self {
        Self { state, wts, kv_constraints, ratelimits }
    }

    /// Handles a syscall
    pub async fn handle_syscall(&self, id: Id, args: SyscallArgs) -> Result<SyscallRet, crate::Error> {
        if self.state.worker_print {
            info!("Executing syscall {args:?}");
        }

        match args {
            SyscallArgs::State { ops } => {
                let res = self.state.mesophyll_client.exec_state_op(id, ops).await?;
                if let Some((ts_new_events, ts_new_flags)) = res.new_tenant_state {
                    self.wts.reload_for_tenant(id, ts_new_events, ts_new_flags, None).map_err(|e| e.to_string())?;
                }

                Ok(SyscallRet::State { res: res.results })
            }
            SyscallArgs::ObjectStorage { op } => {
                let res = op.exec(id, self).await?;
                Ok(SyscallRet::ObjectStorage { res })
            }
            SyscallArgs::Cdn { op } => {
                let res = op.exec(id, self).await?;
                Ok(SyscallRet::Cdn { res })
            }
        }
    }
}