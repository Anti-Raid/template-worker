pub mod cache;
pub mod primitives;
pub mod state;
pub mod template;
mod vm_manager;

use khronos_runtime::utils::khronos_value::KhronosValue;
pub use state::CreateGuildState;
pub use vm_manager::{
    LuaVmAction, LuaVmResult, ThreadClearInactiveGuilds, ThreadMetrics, ThreadRequest, POOL,
};

use serenity::all::GuildId;

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 20; // 20MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 20; // 20MB maximum memory
pub const MAX_TEMPLATES_EXECUTION_TIME: std::time::Duration =
    std::time::Duration::from_secs(60 * 10); // 10 minute maximum execution time
pub const MAX_TEMPLATES_RETURN_WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(60); // 60 seconds maximum execution time

pub const MAX_SERVER_INACTIVITY: std::time::Duration = std::time::Duration::from_secs(600); // 10 minutes till vm marked as inactive

/// Fires an event to all templates associated to a server
/// without waiting for the result.
pub async fn fire(
    guild_id: GuildId,
    state: CreateGuildState,
    action: LuaVmAction,
) -> Result<(), crate::Error> {
    let lua = POOL.get_guild(guild_id, state).await?;

    lua.send(ThreadRequest::Dispatch {
        guild_id,
        action,
        callback: None,
    })
    .map_err(|e| format!("Could not fire event to Lua thread: {}", e))?;

    Ok(())
}

/// Dispatches an event to all templates associated to a server
pub async fn execute(
    guild_id: GuildId,
    state: CreateGuildState,
    action: LuaVmAction,
) -> Result<RenderTemplateHandle, crate::Error> {
    let lua = POOL.get_guild(guild_id, state).await?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();

    lua.send(ThreadRequest::Dispatch {
        guild_id,
        action,
        callback: Some(tx),
    })
    .map_err(|e| format!("Could not send event to Lua thread: {}", e))?;

    Ok(RenderTemplateHandle { rx })
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct MultiLuaVmResultHandle {
    pub results: Vec<LuaVmResultHandle>,
}

impl MultiLuaVmResultHandle {
    #[allow(dead_code)]
    /// Converts the first result to a response if possible, returning an error if the result is an error
    pub fn into_response_first<T: serde::de::DeserializeOwned>(self) -> Result<T, crate::Error> {
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

pub trait IntoResponse
where Self: Sized {
    fn into_response(value: KhronosValue) -> Result<Self, crate::Error>;
    fn into_response_without_types(value: KhronosValue) -> Result<Self, crate::Error>;
}

pub struct KhronosValueResponse(pub KhronosValue);

impl IntoResponse for KhronosValueResponse {
    fn into_response(value: KhronosValue) -> Result<Self, crate::Error> {
        Ok(KhronosValueResponse(value))
    }

    fn into_response_without_types(value: KhronosValue) -> Result<Self, crate::Error> {
        Ok(KhronosValueResponse(value))
    }
}

impl<T: serde::de::DeserializeOwned> IntoResponse for T {
    fn into_response(value: KhronosValue) -> Result<Self, crate::Error> {
        value.into_value::<T>()
    }

    fn into_response_without_types(value: KhronosValue) -> Result<Self, crate::Error> {
        value.into_value_untyped::<T>()
    }
}

impl LuaVmResultHandle {
    /// Convert the result to a response if possible, returning an error if the result is an error
    pub fn into_response<T: IntoResponse>(self) -> Result<T, crate::Error> {
        match self.result {
            LuaVmResult::Ok { result_val } => {
                let res = T::into_response(result_val);
                res
            }
            LuaVmResult::LuaError { err } => Err(format!("Lua error: {}", err).into()),
            LuaVmResult::VmBroken {} => Err("Lua VM is marked as broken".into()),
        }
    }

    /// Convert the result to a response if possible, returning an error if the result is an error
    pub fn into_response_without_types<T: IntoResponse>(self) -> Result<T, crate::Error> {
        match self.result {
            LuaVmResult::Ok { result_val } => {
                let res = T::into_response_without_types(result_val);
                res
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

    #[allow(dead_code)]
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
    rx: tokio::sync::mpsc::UnboundedReceiver<(String, LuaVmResult)>,
}

impl RenderTemplateHandle {
    #[allow(dead_code)]
    /// Wait for the template to render
    pub async fn wait(mut self) -> Result<MultiLuaVmResultHandle, crate::Error> {
        let mut results = Vec::new();
        while let Some((template_name, result)) = self.rx.recv().await {
            results.push(LuaVmResultHandle {
                result,
                template_name,
            });
        }
        
        Ok(MultiLuaVmResultHandle { results })
    }

    /// Wait for the template to render with a timeout
    ///
    /// Returns `None` if the timeout is reached
    pub async fn wait_timeout(
        mut self,
        timeout: std::time::Duration,
    ) -> Result<MultiLuaVmResultHandle, crate::Error> {
        let mut results = Vec::new();
        let mut interval = tokio::time::interval(timeout);
        loop {
            tokio::select! {
                res = self.rx.recv() => {
                    match res {
                        Some((template_name, result)) => {
                            results.push(LuaVmResultHandle {
                                result,
                                template_name,
                            });
                        }
                        None => break, // Channel closed
                    }
                }
                _ = interval.tick() => {
                    log::warn!("Timeout reached while waiting for Lua VM results");
                    break;
                }
            }
        }

        Ok(MultiLuaVmResultHandle { results })
    }
}
