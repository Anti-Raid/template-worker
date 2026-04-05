mod objstorage;
mod cdn;
mod discord;
mod meta;

use std::sync::Arc;

use crate::{geese::{state::{StateExecResult, StateOp}, tenantstate::TenantState}, worker::{limits::{LuaKVConstraints, Ratelimits}, syscall::{cdn::{CdnCall, CdnResult}, discord::ArDiscordProvider, meta::{MetaCall, MetaResult}, objstorage::{ObjectStorageCall, ObjectStorageResult}}, workerstate::WorkerState, workertenantstate::WorkerTenantState, workervmmanager::Id}};
use dapi::context::DiscordContext;
use khronos_runtime::{primitives::lazy::Lazy, rt::mluau::prelude::*};
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
    },
    Discord {
        op: dapi::apilist::API
    },
    Meta {
        op: MetaCall
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

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"State" => {
                let ops = tab.get("ops")?;
                Ok(Self::State { ops })
            },
            b"ObjectStorage" => {
                let op = tab.get("req")?;
                Ok(Self::ObjectStorage { op })
            },
            b"Cdn" => {
                let op = tab.get("req")?;
                Ok(Self::Cdn { op })
            },
            b"Discord" => {
                let op = tab.get("req")?;
                Ok(Self::Discord { op })
            },
            b"Meta" => {
                let op = tab.get("req")?;
                Ok(Self::Meta { op })
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
        res: Vec<StateExecResult>,
        new_tenant_state: Option<TenantState>
    },
    ObjectStorage {
        res: ObjectStorageResult
    },
    Cdn {
        res: CdnResult
    },
    Discord {
        op: &'static str,
        res: serde_json::Value, 
        mrm: dapi::apilist::MapResponseMetadata
    },
    Meta {
        res: MetaResult
    }
}

impl IntoLua for SyscallRet {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table_with_capacity(0, 2)?;
        match self {
            Self::State { res, new_tenant_state } => {
                table.set("op", "State")?;
                table.set("res", res)?;
                table.set("new_tenant_state", new_tenant_state)?;
            }
            Self::ObjectStorage { res } => {
                table.set("op", "ObjectStorage")?;
                table.set("res", res)?;
            }
            Self::Cdn { res } => {
                table.set("op", "Cdn")?;
                table.set("res", res)?;
            }
            Self::Discord { op, res, mrm } => {                
                let res_table = lua.create_table()?;
                res_table.set("op", op)?;
                if mrm.is_primitive_response {
                    let v = lua.to_value_with(&res, khronos_runtime::primitives::LUA_SERIALIZE_OPTIONS)?;
                    if !v.is_null() {
                        res_table.set("res", v)?;
                    }
                } else {
                    let lazy = Lazy::new(res);
                    res_table.set("res", lazy)?;
                }

                table.set("op", "Discord")?;
                table.set("res", res_table)?;
            }
            Self::Meta { res } => {
                table.set("op", "Meta")?;
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
                self.ratelimits.object_storage.check("syscall")?;
                let res = self.state.mesophyll_client.exec_state_op(id, ops).await?;
                if let Some(ref ts) = res.new_tenant_state {
                    self.wts.reload_for_tenant(id, ts).map_err(|e| e.to_string())?;
                }
                Ok(SyscallRet::State { res: res.results, new_tenant_state: res.new_tenant_state })
            }
            SyscallArgs::ObjectStorage { op } => {
                let res = op.exec(id, self).await?;
                Ok(SyscallRet::ObjectStorage { res })
            }
            SyscallArgs::Cdn { op } => {
                let res = op.exec(id, self).await?;
                Ok(SyscallRet::Cdn { res })
            }
            SyscallArgs::Discord { op } => {
                let op_name = op.api_name();
                self.ratelimits.discord.check(op_name)?;
                let dp = DiscordContext::new(ArDiscordProvider { id, state: self.state.clone() });
                let (value, mrm) = op.execute(&dp).await?;
                Ok(SyscallRet::Discord { op: op_name, res: value, mrm })
            }
            SyscallArgs::Meta { op } => {
                let res = op.exec(id, self).await?;
                Ok(SyscallRet::Meta { res })
            }
        }
    }
}