use crate::worker::limits::MAX_TEMPLATES_EXECUTION_TIME;
use crate::worker::limits::MAX_VM_THREAD_STACK_SIZE;
use crate::worker::limits::TEMPLATE_GIVE_TIME;
use khronos_runtime::require::FilesystemWrapper;
use khronos_runtime::rt::mlua::prelude::*;
use khronos_runtime::rt::KhronosIsolate;
use khronos_runtime::rt::KhronosRuntime;
use khronos_runtime::rt::RuntimeCreateOpts;
use khronos_runtime::traits::context::TFlags;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serenity::all::*;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::RwLock;

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
                .build_local(tokio::runtime::LocalOptions::default())
                .expect("Failed to create tokio runtime");

            rt.block_on(async move {
                let mut rt = KhronosRuntime::new(
                    RuntimeCreateOpts {
                        disable_task_lib: false,
                        time_limit: Some(MAX_TEMPLATES_EXECUTION_TIME),
                        give_time: TEMPLATE_GIVE_TIME
                    },
                    None::<(fn(&Lua, LuaThread) -> Result<(), LuaError>, fn() -> ())>,
                )
                .await
                .expect("Failed to create KhronosRuntime");

                rt.sandbox().expect("Failed to create sandbox");

                let subisolate = KhronosIsolate::new_subisolate(
                    rt,
                    FilesystemWrapper::new(crate::worker::builtins::BUILTINS.content.0.clone()),
                    TFlags::READONLY_GLOBALS
                )
                .expect("Failed to create KhronosIsolate");

                let code = subisolate
                    .asset_manager()
                    .get_file("/builtins.register.luau".to_string())
                    .expect("Failed to get asset file");
                let code = String::from_utf8(code)
                    .expect("Failed to convert asset file to string");

                let spawn_result = subisolate
                    .spawn_script(
                        "/builtins.register.luau",
                        "/builtins.register.luau",
                        &code,
                        khronos_runtime::rt::mlua::MultiValue::with_capacity(0)
                    )
                    .await
                    .expect("Failed to spawn asset");

                let result = spawn_result
                    .into_khronos_value(&subisolate)
                    .expect("Failed to convert result to serde_json_value");

                let result: RegisterResult = result
                    .into_value()
                    .expect("Failed to deserialize RegisterResult");

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
