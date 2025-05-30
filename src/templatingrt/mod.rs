pub mod cache;
pub mod primitives;
pub mod state;
pub mod template;
mod vm_manager;

pub use state::CreateGuildState;
pub use vm_manager::{LuaVmAction, LuaVmResult, DEFAULT_THREAD_POOL, ThreadMetrics, ThreadGuildVmMetrics};

use serenity::all::GuildId;
use crate::templatingrt::vm_manager::ArLuaHandle;
use primitives::sandwich_config;
pub use vm_manager::get_lua_vm;

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 20; // 20MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 20; // 20MB maximum memory
pub const MAX_TEMPLATES_EXECUTION_TIME: std::time::Duration =
    std::time::Duration::from_secs(60 * 5); // 5 minute maximum execution time
pub const MAX_TEMPLATES_RETURN_WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(10); // 10 seconds maximum execution time

pub const MAX_SERVER_INACTIVITY: std::time::Duration = std::time::Duration::from_secs(600); // 10 minutes till vm marked as inactive
pub const MAX_SERVER_INACTIVITY_CHECK_TIME: std::time::Duration = std::time::Duration::from_secs(60 * 15); // Check for inactive servers every 15 minutes

/// Dispatches an event to all templates associated to a server
pub async fn execute(
    guild_id: GuildId,
    state: CreateGuildState,
    action: LuaVmAction,
) -> Result<RenderTemplateHandle, silverpelt::Error> {
    let lua = get_lua_vm(
        guild_id,
        state
    )
    .await?;

    let (tx, rx) = tokio::sync::oneshot::channel();

    lua.send_action(action, tx)
        .map_err(|e| format!("Could not send event to Lua thread: {}", e))?;

    Ok(RenderTemplateHandle { rx })
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct MultiLuaVmResultHandle {
    pub results: Vec<LuaVmResultHandle>,
}

impl MultiLuaVmResultHandle {
    /// Converts the first result to a response if possible, returning an error if the result is an error
    pub fn into_response_first<T: serde::de::DeserializeOwned>(
        self,
    ) -> Result<T, silverpelt::Error> {
        let Some(result) = self.results.into_iter().next() else {
            return Err("No results".into());
        };

        result.into_response::<T>()
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct LuaVmResultHandle {
    pub result: LuaVmResult,
    pub template_name: String,
}

impl LuaVmResultHandle {
    /// Convert the result to a response if possible, returning an error if the result is an error
    pub fn into_response<T: serde::de::DeserializeOwned>(self) -> Result<T, silverpelt::Error> {
        match self.result {
            LuaVmResult::Ok { result_val } => {
                let res = serde_json::from_value(result_val)?;
                Ok(res)
            }
            LuaVmResult::LuaError { err } => Err(format!("Lua error: {}", err).into()),
            LuaVmResult::VmBroken {} => Err("Lua VM is marked as broken".into()),
        }
    }

    #[allow(dead_code)]
    /// Returns ``true`` if the result is an LuaError or VmBroken
    pub fn is_error(&self) -> bool {
        matches!(
            self.result,
            LuaVmResult::LuaError { .. } | LuaVmResult::VmBroken {}
        )
    }

    #[allow(dead_code)]
    /// Returns ``true`` if the result is caused by a broken VM
    pub fn is_vm_broken(&self) -> bool {
        matches!(self.result, LuaVmResult::VmBroken {})
    }

    /// Returns the inner error if the result is an error
    pub fn lua_error(&self) -> Option<&str> {
        match &self.result {
            LuaVmResult::LuaError { err } => Some(err),
            LuaVmResult::VmBroken {} => Some("Lua VM is marked as broken"),
            _ => None,
        }
    }
}

/// A handle to allow waiting for a template to render
pub struct RenderTemplateHandle {
    rx: tokio::sync::oneshot::Receiver<Vec<(String, LuaVmResult)>>,
}

impl RenderTemplateHandle {
    /// Wait for the template to render
    pub async fn wait(self) -> Result<MultiLuaVmResultHandle, silverpelt::Error> {
        let res = self.rx.await?;
        let res = res
            .into_iter()
            .map(|(name, result)| LuaVmResultHandle {
                result,
                template_name: name,
            })
            .collect::<Vec<_>>();

        Ok(MultiLuaVmResultHandle { results: res })
    }

    /// Wait for the template to render with a timeout
    ///
    /// Returns `None` if the timeout is reached
    pub async fn wait_timeout(
        self,
        timeout: std::time::Duration,
    ) -> Result<Option<MultiLuaVmResultHandle>, silverpelt::Error> {
        match tokio::time::timeout(timeout, self.rx).await {
            Ok(Ok(res)) => {
                let res = res
                    .into_iter()
                    .map(|(name, result)| LuaVmResultHandle {
                        result,
                        template_name: name,
                    })
                    .collect::<Vec<_>>();
                Ok(Some(MultiLuaVmResultHandle { results: res }))
            }
            Ok(Err(e)) => Err(format!("Could not receive data from Lua thread: {}", e).into()),
            Err(_) => Ok(None),
        }
    }
}
