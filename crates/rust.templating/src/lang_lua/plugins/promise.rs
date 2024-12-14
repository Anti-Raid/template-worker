use mlua::prelude::*;
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

#[derive(Clone)]
pub struct TestPromise {
    pub i: std::rc::Rc<usize>,
}

impl LuaUserData for TestPromise {
    fn add_fields<F: LuaUserDataFields<Self>>(fields: &mut F) {
        fields.add_field_method_get("my_promise", |_lua, t| {
            let t = t.clone();
            let promise = LuaPromise::new_generic(move |_lua| {
                let t = t.clone();
                async move {
                    if 0 == 1 {
                        return Err(mlua::Error::RuntimeError("Error".to_string()));
                    }
                    if *t.i != 0 {
                        return Ok(*t.i);
                    }

                    Ok(0)
                }
            });

            Ok(promise)
        });
    }
}

pub type LuaPromiseRef = LuaUserDataRefMut<LuaPromise>;

impl LuaUserData for LuaPromise {}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "yield",
        lua.create_async_function(|lua, promise: LuaPromiseRef| async move {
            let fut = (promise.inner)(lua);
            fut.await
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
