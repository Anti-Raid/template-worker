use mlua::prelude::*;
use mlua_scheduler::LuaSchedulerAsync;
use std::{future::Future, pin::Pin};

pub type LuaPromiseFut = Pin<Box<dyn Future<Output = LuaResult<LuaMultiValue>>>>;

/// Represents a promise that must be yielded to get the result.
///
/// LuaPromise's are not run at all until ``promise.yield`` is called
/// in Lua code
pub struct LuaPromise {
    pub inner: Box<dyn Fn(Lua) -> LuaPromiseFut>, // Box the stream to ensure its pinned,
}

impl LuaPromise {
    #[allow(dead_code)]
    pub fn new(fut: Box<dyn Fn(Lua) -> LuaPromiseFut>) -> Self {
        Self { inner: fut }
    }

    pub fn new_generic<
        T: Future<Output = LuaResult<R>> + 'static,
        U: Fn(&Lua) -> T + Clone + 'static,
        R: IntoLuaMulti + 'static,
    >(
        func: U,
    ) -> Self {
        Self {
            inner: Box::new(move |lua| {
                let func_ref = func.clone();
                Box::pin(async move {
                    let fut = async move {
                        let fut = (func_ref)(&lua);
                        match fut.await {
                            Ok(val) => val.into_lua_multi(&lua),
                            Err(e) => Err(e),
                        }
                    };

                    fut.await
                })
            }),
        }
    }
}

/// Macro lua_promise!(arg1, arg2: Type2, |lua, {args}|, {
///     // Future code
/// })
///
/// Creates:
///
/// LuaPromise::new_generic(move |lua| {
///     let arg1 = arg1.clone();
///     let arg2 = arg2.clone();
///   
///     async move {
///        let c = |lua, arg1, arg2| {
///          // Future code
///        };
///
///        c(lua, args).await    
///    }
/// })
///
/// Clones all arguments and the lua instance
macro_rules! lua_promise {
    ($($arg:ident),* $(,)?, |$lua:ident, $($args:ident),*|, $code:block) => {
        {
            use crate::lang_lua::plugins::promise::LuaPromise;
            // let arg1 = arg1.clone();
            // let arg2 = arg2.clone();
            $(
                let $arg = $arg.clone();
            )*

            LuaPromise::new_generic(move |$lua| {
                // let arg1 = arg1.clone();
                // let arg2 = arg2.clone();
                // ...
                $(
                    let $arg = $arg.clone();
                )*
                let $lua = $lua.clone();

                async move {
                    $(
                        let $args = $args.clone();
                    )*

                    let $lua = $lua.clone();

                    $code
                }
            })
        }
    };
}
pub(super) use lua_promise;

pub type LuaPromiseRef = LuaUserDataRefMut<LuaPromise>;

impl LuaUserData for LuaPromise {}

pub fn plugin_docs() -> templating_docgen::Plugin {
    templating_docgen::Plugin::default()
        .name("@antiraid/promise")
        .description("Lua Promises, yield for a promise to execute the async action returning its result.")
        .type_mut(
            "LuaPromise",
            "LuaPromise<T> provides a promise that must be yielded to actually execute and get the result of the async action.",
            |t| {
                t
                .add_generic("T")
            }
        )
        .method_mut("yield", |m| {
            m.description("Yields the promise to execute the async action and return its result. Note that this is the only function other than `stream.next` that yields.")
            .parameter("promise", |p| {
                p.typ("LuaPromise<T>").description("The promise to yield.")
            })
            .return_("T", |r| {
                r.typ("T").description("The result of executing the promise.")
            })
        })
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "yield",
        lua.create_scheduler_async_function(|lua, promise: LuaPromiseRef| async move {
            let fut = (promise.inner)(lua);
            fut.await
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
