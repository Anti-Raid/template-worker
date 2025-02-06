pub mod cache;
pub mod primitives;
pub mod state;
pub mod template;
mod vm_manager;

use khronos_runtime::primitives::event::CreateEvent;
use primitives::sandwich_config;
use serenity::all::GuildId;
use std::sync::{atomic::Ordering, Arc};
use template::Template;
use vm_manager::{get_lua_vm, LuaVmAction, LuaVmResult};

use crate::config::CONFIG;

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 3; // 3MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 8; // 8MB maximum memory
pub const MAX_TEMPLATES_EXECUTION_TIME: std::time::Duration =
    std::time::Duration::from_secs(60 * 5); // 5 minute maximum execution time
pub const MAX_TEMPLATES_RETURN_WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(10); // 10 seconds maximum execution time

/// Render a template given an event, state and template
///
/// Pre-conditions: the serenity context's shard matches the guild itself
pub async fn execute(
    event: CreateEvent,
    state: ParseCompileState,
    template: Arc<Template>,
) -> Result<RenderTemplateHandle, silverpelt::Error> {
    let lua = get_lua_vm(
        state.guild_id,
        state.pool,
        state.serenity_context,
        state.reqwest_client,
    )
    .await?;

    // Update last execution time.
    lua.last_execution_time
        .store(std::time::Instant::now(), Ordering::Release);

    let (tx, rx) = tokio::sync::oneshot::channel();

    lua.handle
        .send((LuaVmAction::Exec { template, event }, tx))
        .map_err(|e| format!("Could not send data to Lua thread: {}", e))?;

    Ok(RenderTemplateHandle { rx })
}

pub struct LuaVmResultHandle {
    result: LuaVmResult,
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
        template_name: &str,
        guild_id: serenity::all::GuildId,
        serenity_context: &serenity::all::Context,
    ) -> Result<(), silverpelt::Error> {
        match self.result {
            LuaVmResult::VmBroken {} => {
                log::error!("Lua VM is broken in template {}", template_name);
                log_error(
                    guild_id,
                    serenity_context,
                    template_name,
                    "Lua VM has been marked as broken".to_string(),
                )
                .await?;
            }
            LuaVmResult::LuaError { ref err } => {
                log::error!("Lua error in template {}: {}", template_name, err);

                log_error(guild_id, serenity_context, template_name, err.to_string()).await?;
            }
            _ => {}
        }

        Ok(())
    }
}

/// A handle to allow waiting for a template to render
pub struct RenderTemplateHandle {
    rx: tokio::sync::oneshot::Receiver<LuaVmResult>,
}

impl RenderTemplateHandle {
    /// Wait for the template to render
    pub async fn wait(self) -> Result<LuaVmResultHandle, silverpelt::Error> {
        let res = self
            .rx
            .await
            .map_err(|e| format!("Could not receive data from Lua thread: {}", e))?;

        Ok(LuaVmResultHandle { result: res })
    }

    /// Waits for the template to render, then logs an error if the result is an error
    pub async fn wait_and_log_error(
        self,
        template_name: &str,
        guild_id: serenity::all::GuildId,
        serenity_context: &serenity::all::Context,
    ) -> Result<LuaVmResultHandle, silverpelt::Error> {
        let res = self.wait().await?;
        res.log_error(template_name, guild_id, serenity_context)
            .await?;
        Ok(res)
    }

    /// Wait for the template to render with a timeout
    pub async fn wait_timeout(
        self,
        timeout: std::time::Duration,
    ) -> Result<Option<LuaVmResultHandle>, silverpelt::Error> {
        match tokio::time::timeout(timeout, self.rx).await {
            Ok(Ok(res)) => Ok(Some(LuaVmResultHandle { result: res })),
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

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FireBenchmark {
    pub hashmap_insert_time: u128,
    pub get_lua_vm: u128,
    pub exec_simple: u128,
    pub exec_no_wait: u128,
    pub exec_error: u128,
}

/// Benchmark the Lua VM
pub async fn benchmark_vm(
    guild_id: GuildId,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
) -> Result<FireBenchmark, silverpelt::Error> {
    // Get_lua_vm
    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    let _ = get_lua_vm(guild_id_a, pool_a, serenity_context_a, reqwest_client_a).await?;
    let get_lua_vm = start.elapsed().as_micros();

    let new_map = scc::HashMap::new();
    let start = std::time::Instant::now();
    let _ = new_map.insert_async(1, 1).await;
    let hashmap_insert_time = start.elapsed().as_micros();

    // Exec simple with wait

    let pt = Template {
        content: "return 1".to_string(),
        ..Default::default()
    };

    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    let n: i32 = execute(
        CreateEvent::new(
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            serde_json::Value::Null,
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt.into(),
    )
    .await?
    .wait()
    .await?
    .into_response()?;

    let exec_simple = start.elapsed().as_micros();

    if n != 1 {
        return Err(format!("Expected 1, got {}", n).into());
    }

    // Exec simple with no wait
    let pt = Template {
        content: "return 1".to_string(),
        ..Default::default()
    };

    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    execute(
        CreateEvent::new(
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            serde_json::Value::Null,
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt.into(),
    )
    .await?;
    let exec_no_wait = start.elapsed().as_micros();

    // Exec simple with wait
    let pt = Template {
        content: "error('MyError')\nreturn 1".to_string(),
        ..Default::default()
    };

    let pool_a = pool.clone();
    let guild_id_a = guild_id;
    let serenity_context_a = serenity_context.clone();
    let reqwest_client_a = reqwest_client.clone();

    let start = std::time::Instant::now();
    let err = execute(
        CreateEvent::new(
            "Benchmark".to_string(),
            "Benchmark".to_string(),
            serde_json::Value::Null,
            None,
        ),
        ParseCompileState {
            serenity_context: serenity_context_a,
            reqwest_client: reqwest_client_a,
            guild_id: guild_id_a,
            pool: pool_a,
        },
        pt.into(),
    )
    .await?
    .wait()
    .await?;
    let exec_error = start.elapsed().as_micros();

    let Some(err) = err.lua_error() else {
        return Err("Expected error, got success".into());
    };

    if !err.contains("MyError") {
        return Err(format!("Expected MyError, got {}", err).into());
    }

    Ok(FireBenchmark {
        get_lua_vm,
        hashmap_insert_time,
        exec_simple,
        exec_no_wait,
        exec_error,
    })
}

#[derive(Clone)]
pub struct ParseCompileState {
    pub serenity_context: serenity::all::Context,
    pub reqwest_client: reqwest::Client,
    pub guild_id: GuildId,
    pub pool: sqlx::PgPool,
}
