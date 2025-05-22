pub mod cache;
pub mod primitives;
pub mod state;
pub mod template;
mod vm_manager;

pub use state::CreateGuildState;
pub use vm_manager::{LuaVmAction, LuaVmResult, DEFAULT_THREAD_POOL};

use serenity::all::GuildId;
use crate::templatingrt::vm_manager::ArLuaHandle;
use primitives::sandwich_config;
use template::Template;
pub use vm_manager::get_lua_vm;

use crate::config::CONFIG;

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 20; // 20MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 20; // 20MB maximum memory
pub const MAX_TEMPLATES_EXECUTION_TIME: std::time::Duration =
    std::time::Duration::from_secs(60 * 5); // 5 minute maximum execution time
pub const MAX_TEMPLATES_RETURN_WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(10); // 10 seconds maximum execution time

/// Dispatches an event to all templates associated to a server
///
/// Pre-conditions: the serenity context's shard matches the guild itself
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

    /// Logs an error in the case of a error lua vm result
    pub async fn log_error(
        &self,
        guild_id: serenity::all::GuildId,
        serenity_context: &serenity::all::Context,
    ) -> Result<(), silverpelt::Error> {
        match self.result {
            LuaVmResult::VmBroken {} => {
                log::error!("Lua VM is broken in template {}", &self.template_name);
                log_error(
                    guild_id,
                    serenity_context,
                    &self.template_name,
                    "Lua VM has been marked as broken".to_string(),
                )
                .await?;
            }
            LuaVmResult::LuaError { ref err } => {
                log::error!("Lua error in template {}: {}", &self.template_name, err);

                log_error(
                    guild_id,
                    serenity_context,
                    &self.template_name,
                    err.to_string(),
                )
                .await?;
            }
            _ => {}
        }

        Ok(())
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

    /// Waits for the template to render, then logs an error if the result is an error
    pub async fn wait_and_log_error(
        self,
        guild_id: serenity::all::GuildId,
        serenity_context: &serenity::all::Context,
    ) -> Result<MultiLuaVmResultHandle, silverpelt::Error> {
        let res = self.wait().await?;
        for result in &res.results {
            if result.is_error() {
                result.log_error(guild_id, serenity_context).await?;
            }
        }
        Ok(res)
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

/// Helper method to get guild template and log error
///
/// Equivalent to calling `get_guild_template` to get the template and then calling `dispatch_error`
pub async fn log_error(
    guild_id: serenity::all::GuildId,
    serenity_context: &serenity::all::Context,
    template_name: &str,
    e: String,
) -> Result<(), silverpelt::Error> {
    log::error!("Lua thread error: {}: {}", template_name, e);

    let Some(template) = cache::get_guild_template(guild_id, template_name).await else {
        return Err("Failed to get template data for error reporting".into());
    };

    dispatch_error(serenity_context, &e, guild_id, &template).await
}

/// Dispatches an error to a channel
pub async fn dispatch_error(
    ctx: &serenity::all::Context,
    error: &str,
    guild_id: serenity::all::GuildId,
    template: &Template,
) -> Result<(), silverpelt::Error> {
    // Codeblock + escape the error string
    let error = format!("```lua\n{}```", error.replace('`', "\\`"));

    let data = ctx.data::<silverpelt::data::Data>();

    if let Some(error_channel) = template.error_channel {
        let Some(channel) = sandwich_driver::channel(
            &ctx.cache,
            &ctx.http,
            &data.reqwest,
            Some(guild_id),
            error_channel,
            &sandwich_config(),
        )
        .await?
        else {
            return Ok(());
        };

        let Some(guild_channel) = channel.guild() else {
            return Ok(());
        };

        if guild_channel.guild_id != guild_id {
            return Ok(());
        }

        guild_channel
            .send_message(
                &ctx.http,
                serenity::all::CreateMessage::new()
                    .embed(
                        serenity::all::CreateEmbed::new()
                            .title("Error executing template")
                            .field("Error", error, false)
                            .field("Template", template.name.clone(), false),
                    )
                    .components(vec![serenity::all::CreateActionRow::Buttons(
                        vec![serenity::all::CreateButton::new_link(
                            &CONFIG.meta.support_server_invite,
                        )
                        .label("Support Server")]
                        .into(),
                    )]),
            )
            .await?;
    }

    Ok(())
}