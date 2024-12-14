use futures_util::{Stream, StreamExt};
use mlua::prelude::*;
use std::pin::Pin;

pub type StreamValue = Box<dyn FnOnce(&Lua) -> LuaResult<LuaValue>>;
pub type LuaStreamFut = Pin<Box<dyn Stream<Item = StreamValue>>>;

pub struct LuaStream {
    pub inner: LuaStreamFut, // Box the stream to ensure its pinned,
}

impl LuaStream {
    pub fn new(stream: LuaStreamFut) -> Self {
        Self { inner: stream }
    }
}

pub type LuaStreamRef = LuaUserDataRefMut<LuaStream>;

impl LuaUserData for LuaStream {}

pub fn plugin_docs() -> templating_docgen::Plugin {
    templating_docgen::Plugin::default()
        .name("@antiraid/stream")
        .description("Lua Streams, yield for a set of values using next.")
        .type_mut(
            "LuaStream",
            "LuaStream<T> provides a stream implementation. This is returned by MessageHandle's await_component_interaction for instance for handling button clicks/select menu choices etc.",
            |t| {
                t
                .add_generic("T")
            }
        )
        .method_mut("next", |m| {
            m.description("Returns the next value in the stream. Note that this is the only function other than `promise.yield` that yields.")
            .parameter("stream", |p| {
                p.typ("LuaStream<T>").description("The stream to get the next value from.")
            })
            .return_("T", |r| {
                r.typ("T").description("The next value in the stream, or nil if the stream is exhausted.")
            })
        })
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "next",
        lua.create_async_function(|lua, mut stream: LuaStreamRef| async move {
            match stream.inner.next().await {
                Some(item) => match item(&lua) {
                    Ok(value) => Ok(value),
                    Err(e) => Err(e),
                },
                None => Ok(LuaValue::Nil), // Return nil if the stream is exhausted
            }
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
