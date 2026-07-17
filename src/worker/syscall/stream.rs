use khronos_ext::mlua_scheduler_ext::LuaSchedulerAsyncUserData;
use khronos_runtime::{core::datetime::TimeDelta, rt::mluau::prelude::*, utils::khronos_value::KhronosValue};
use rand::distr::{Alphanumeric, SampleString};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{geese::ratelimit::RlExceededError, worker::{syscall::SyscallHandler, workervmmanager::Id}};

/// Stream syscalls
#[derive(Debug)]
pub enum StreamCall {
    NewStream {},
}

impl FromLua for StreamCall {
    fn from_lua(value: LuaValue, _lua: &Lua) -> LuaResult<Self> {
        let LuaValue::Table(tab) = value else {
            return Err(LuaError::FromLuaConversionError {
                from: value.type_name(),
                to: "StreamCall".to_string(),
                message: Some("expected a table".to_string()),
            })
        };

        let typ: LuaString = tab.get("op")?;
        match typ.as_bytes().as_ref() {
            b"NewStream" => {
                Ok(Self::NewStream { })
            },
            _ => {
                Err(LuaError::FromLuaConversionError {
                    from: "table",
                    to: "StreamCall".to_string(),
                    message: Some("invalid op provided".to_string()),
                })
            }
        }
    }
}

pub struct Stream {
    id: String,
    send: UnboundedSender<KhronosValue>,
    recv: UnboundedReceiver<KhronosValue>
}

impl LuaUserData for Stream {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("id", |lua, this, _: ()| {
            lua.create_string(&this.id)
        });

        methods.add_method("send", |_, this, v: KhronosValue| {
            this.send.send(v).map_err(|x| LuaError::external(x.to_string()))?;
            Ok(())
        });

        methods.add_scheduler_async_method_mut("recv", async |_, mut this, timeout: Option<LuaUserDataRef<TimeDelta>>| {
            match timeout {
                Some(t) => {
                    let timeout = t.timedelta.to_std().map_err(LuaError::external)?;
                    let res = tokio::time::timeout(timeout, this.recv.recv()).await.map_err(|x| LuaError::external(x.to_string()))?;
                    Ok(res)
                }
                None => {
                    Ok(this.recv.recv().await)
                }
            }
        });

        methods.add_method_mut("close", |_, this, _: ()| {
            this.recv.close();
            Ok(())
        });
    }
}

pub enum StreamResult {
    Stream {
        stream: Stream
    }
}

impl IntoLua for StreamResult {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        let table = lua.create_table()?;
        match self {
            Self::Stream { stream } => {
                table.set("op", "Stream")?;
                table.set("stream", stream)?;
            },
        }
        table.set_readonly(true); // We want StateExecResult's to be immutable
        Ok(LuaValue::Table(table))
    }
}

impl StreamCall {
    pub(super) async fn exec(self, _id: Id, handler: &SyscallHandler) -> Result<StreamResult, crate::Error> {
        match self {
            Self::NewStream {} => {
                handler.ratelimits.runtime.check("NewStream", ()).map_err(RlExceededError)?;
                let stream_id = format!("{};{}", handler.state.mesophyll_client.worker_id, Alphanumeric.sample_string(&mut rand::rng(), 96));
                // Attach stream id to client
                let (send, recv) = handler.state.mesophyll_client.attach_stream(stream_id.clone()).await;
                Ok(StreamResult::Stream {
                    stream: Stream { id: stream_id, send, recv }
                })
            }
        }
    }
}