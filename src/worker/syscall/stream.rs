use khronos_runtime::{rt::mluau::prelude::*, utils::khronos_value::KhronosValue};

use crate::{geese::{ratelimit::RlExceededError, stream::StreamId}, worker::{syscall::SyscallHandler, workervmmanager::Id}};

/// Stream syscalls
#[derive(Debug)]
pub enum StreamCall {
    New {},
    Send {
        stream: StreamId,
        value: KhronosValue
    }
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
            b"New" => {
                Ok(Self::New { })
            },
            b"Send" => {
                let stream = tab.get("stream")?;
                let value = tab.get("value")?;
                Ok(Self::Send { stream, value })
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

pub enum StreamResult {
    Stream {
        stream: StreamId
    },
    Sent,
}

impl IntoLua for StreamResult {
    fn into_lua(self, lua: &Lua) -> LuaResult<LuaValue> {
        match self {
            Self::Stream { stream } => {
            let table = lua.create_table()?;
                table.set("op", "Stream")?;
                table.set("stream_id", stream)?;
                table.set_readonly(true);
                Ok(LuaValue::Table(table))
            },
            Self::Sent {} => {
                Ok(LuaNil)
            }
        }
    }
}

impl StreamCall {
    pub(super) async fn exec(self, _id: Id, handler: &SyscallHandler) -> Result<StreamResult, crate::Error> {
        match self {
            Self::New {} => {
                handler.ratelimits.runtime.check("NewStream", ()).map_err(RlExceededError)?;
                let sid = StreamId::new_rand(handler.state.mesophyll_client.worker_id as u64);
                handler.state.mesophyll_client.attach_stream(sid);
                Ok(StreamResult::Stream {
                    stream: sid
                })
            },
            Self::Send { stream, value } => {
                handler.state.mesophyll_client.stream_message(stream, value)?;
                Ok(StreamResult::Sent)
            }
        }
    }
}