use antiraid_types::ar_event::AntiraidEvent;
use khronos_runtime::primitives::event::Event as KEvent;
use khronos_runtime::require::FilesystemWrapper;
use khronos_runtime::rt::mlua::prelude::*;
use khronos_runtime::rt::KhronosIsolate;
use khronos_runtime::rt::KhronosRuntime;
use khronos_runtime::rt::KhronosRuntimeInterruptData;
use khronos_runtime::rt::RuntimeCreateOpts;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::all::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;
use std::time::Duration;

use crate::dispatch::parse_event;
use crate::templatingrt::primitives::dummyctx::DummyProvider;
use crate::templatingrt::MAX_VM_THREAD_STACK_SIZE;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommand {
    #[serde(rename = "type")]
    pub kind: Option<CommandType>,
    pub handler: Option<EntryPointHandlerType>,

    pub name: Option<String>,
    pub name_localizations: HashMap<String, String>,
    pub description: Option<String>,
    pub description_localizations: HashMap<String, String>,
    pub default_member_permissions: Option<Permissions>,
    pub dm_permission: Option<bool>,
    pub integration_types: Option<Vec<InstallationContext>>,
    pub contexts: Option<Vec<InteractionContext>>,
    pub nsfw: bool,
    pub options: Vec<CreateCommandOption>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommandOption {
    #[serde(rename = "type")]
    pub kind: CommandOptionType,
    pub name: String,
    pub name_localizations: Option<HashMap<String, String>>,
    pub description: String,
    pub description_localizations: Option<HashMap<String, String>>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub choices: Vec<CreateCommandOptionChoice>,
    #[serde(default)]
    pub options: Vec<CreateCommandOption>,
    #[serde(default)]
    pub channel_types: Vec<ChannelType>,
    #[serde(default)]
    pub min_value: Option<serde_json::Number>,
    #[serde(default)]
    pub max_value: Option<serde_json::Number>,
    #[serde(default)]
    pub min_length: Option<u16>,
    #[serde(default)]
    pub max_length: Option<u16>,
    #[serde(default)]
    pub autocomplete: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateCommandOptionChoice {
    pub name: String,
    pub name_localizations: Option<HashMap<String, String>>,
    pub value: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterResult {
    pub commands: Vec<CreateCommand>,
}

pub static REGISTER: LazyLock<RegisterResult> =
    LazyLock::new(|| register().expect("Failed to register builtins"));

fn register() -> Result<RegisterResult, crate::Error> {
    let result = Arc::new(RwLock::new(None::<RegisterResult>));

    let ref_a = result.clone();
    std::thread::Builder::new()
        .stack_size(MAX_VM_THREAD_STACK_SIZE) // Increase stack size for the thread
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build_local(&tokio::runtime::LocalOptions::default())
                .expect("Failed to create tokio runtime");

            rt.block_on(async move {
                let mut rt = KhronosRuntime::new(
                    RuntimeCreateOpts {
                        disable_task_lib: false,
                    },
                    Some(|_a: &Lua, b: &KhronosRuntimeInterruptData| {
                        let Some(last_execution_time) = b.last_execution_time else {
                            return Ok(LuaVmState::Continue);
                        };

                        // In builtins.register.luau, we only give 2 seconds for the code to register
                        if last_execution_time.elapsed() >= Duration::from_secs(2) {
                            return Ok(LuaVmState::Yield);
                        }

                        Ok(LuaVmState::Continue)
                    }),
                    None::<(fn(&Lua, LuaThread) -> Result<(), LuaError>, fn() -> ())>,
                )
                .expect("Failed to create KhronosRuntime");

                rt.sandbox().expect("Failed to create sandbox");

                let subisolate = KhronosIsolate::new_subisolate(
                    rt,
                    FilesystemWrapper::new(crate::templatingrt::cache::BUILTINS.content.0.clone()),
                )
                .expect("Failed to create KhronosIsolate");

                tokio::task::yield_now().await;

                // Create dummy context with BuiltinsRegisterDataStore
                let provider = DummyProvider::new(vec![]);

                let created_context = subisolate
                    .create_context(provider)
                    .expect("Failed to create context");

                let event =
                    parse_event(&AntiraidEvent::OnStartup(vec![])).expect("Failed to parse event");

                let spawn_result = subisolate
                    .spawn_asset(
                        "/builtins.register.luau",
                        "/builtins.register.luau",
                        created_context,
                        KEvent::from_create_event(&event),
                    )
                    .await
                    .expect("Failed to spawn asset");

                let result = spawn_result
                    .into_serde_json_value(&subisolate)
                    .expect("Failed to convert result to serde_json_value");

                let result: RegisterResult =
                    serde_json::from_value(result).expect("Failed to deserialize RegisterResult");

                // Store the result in the shared Arc<RwLock>
                let mut result_lock = ref_a.write().unwrap();
                *result_lock = Some(result);

                subisolate.inner().scheduler().stop();
            });
        })
        .expect("Failed to spawn register thread")
        .join()
        .map_err(|e| {
            let mut payload = "Unknown error in register thread".to_string();
            if let Some(err) = e.downcast_ref::<String>() {
                payload = err.clone();
            } else if let Some(err) = e.downcast_ref::<&str>() {
                payload = err.to_string();
            }

            format!("Failed to join register thread: {}", payload)
        })?;

    // Assume result is populated by the thread
    let result = result.read().unwrap();

    if let Some(register_result) = result.as_ref() {
        Ok(register_result.clone())
    } else {
        Err("No register result available".into())
    }
}
