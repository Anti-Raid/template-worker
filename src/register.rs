use crate::fauxpas::mainthread::{run_in_thread, RunInThreadFn};
use crate::worker::builtins::{Builtins, BuiltinsPatches, TemplatingTypes};
use dapi::types::CreateCommand;
use khronos_runtime::rt::mlua::prelude::*;
use khronos_runtime::rt::KhronosRuntime;
use serde::{Deserialize, Serialize};
use serenity::all::*;
use std::sync::LazyLock;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegisterResult {
    pub commands: Vec<CreateCommand>,
}

pub static REGISTER: LazyLock<RegisterResult> =
    LazyLock::new(|| register().expect("Failed to register builtins"));

fn register() -> Result<RegisterResult, crate::Error> {
    struct RunInThreadRegister;
    impl RunInThreadFn<(), RegisterResult> for RunInThreadRegister {
        async fn run(rt: &KhronosRuntime, _data: ()) -> RegisterResult {
            let builtins_register = rt
                .eval_script::<LuaValue>(
                    "./builtins.register",
                )
                .expect("Failed to spawn asset");

            let result: RegisterResult = rt.from_value(builtins_register)
                .expect("Failed to deserialize RegisterResult");

            result
        }
    }

    Ok(run_in_thread::<RunInThreadRegister, _, _, _>(
    vfs::OverlayFS::new(&vec![
            vfs::EmbeddedFS::<BuiltinsPatches>::new().into(),
            vfs::EmbeddedFS::<Builtins>::new().into(),
            vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
        ]),
        ()
    ))
}
