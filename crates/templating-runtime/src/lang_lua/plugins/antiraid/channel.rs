#![allow(clippy::await_holding_refcell_ref)]

use std::{cell::RefCell, rc::Rc};

use mlua::prelude::*;
use tokio::sync::broadcast::{Receiver, Sender};

use super::promise::lua_promise;

pub struct LuaChannelSender {
    pub sender: Sender<LuaValue>,
}

impl LuaUserData for LuaChannelSender {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method_mut("send", |_lua, this, data: LuaValue| {
            this.sender
                .send(data)
                .map_err(|e| LuaError::external(e.to_string()))
        });

        // Returns true if no queued values
        methods.add_method("is_empty", |_lua, this, _: ()| Ok(this.sender.is_empty()));

        // Number of queued values
        methods.add_method("len", |_lua, this, _: ()| Ok(this.sender.len()));

        // Number of recievers
        methods.add_method("reciever_count", |_lua, this, _: ()| {
            Ok(this.sender.receiver_count())
        });

        // Returns true if two LuaChannelSender instances belong to the same channel
        methods.add_method(
            "same_channel",
            |_lua, this, other: LuaUserDataRef<LuaChannelSender>| {
                Ok(this.sender.same_channel(&other.sender))
            },
        );

        // Creates a new reciever for the sender
        methods.add_method("subscribe", |_lua, this, _: ()| {
            let reciever = LuaChannelReciever {
                reciever: Rc::new(this.sender.subscribe().into()),
            };

            Ok(reciever)
        });
    }
}

#[derive(Clone)]
pub struct LuaChannelReciever {
    pub reciever: Rc<RefCell<Receiver<LuaValue>>>,
}

impl LuaUserData for LuaChannelReciever {
    fn add_methods<M: LuaUserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("recv", |_lua, this, timeout_secs: u64| {
            Ok(lua_promise!(this, timeout_secs, |_lua, this, timeout_secs|, {
                let timeout = std::time::Duration::from_secs(timeout_secs);
                if timeout > crate::MAX_TEMPLATES_EXECUTION_TIME {
                    return Err(LuaError::external(format!("Timeout cannot be greater than {:?}", crate::MAX_TEMPLATES_EXECUTION_TIME)));
                }

                // SAFETY: All borrows use the non-panicking try_borrow* methods.
                // Hence, there is no risk of a panic while holding a borrow.
                let mut reciever = this.reciever.try_borrow_mut()
                    .map_err(|e| LuaError::RuntimeError(
                        format!("{}. Note that a Reciever cannot recieve concurrently", e)
                    ))?;

                tokio::time::timeout(timeout, reciever.recv())
                    .await
                    .map_err(|e| LuaError::external(format!("Timeout: {}", e)))?
                    .map_err(|e| LuaError::external(e.to_string()))
            }))
        });

        // Returns true if there aren’t any messages in the channel that the Receiver has yet to receive.
        methods.add_method("is_empty", |_lua, this, _: ()| {
            let reciever = this.reciever.try_borrow().map_err(|e| {
                LuaError::RuntimeError(format!(
                    "{}. Note that a Reciever cannot recieve concurrently",
                    e
                ))
            })?;

            Ok(reciever.is_empty())
        });

        // Returns the number of messages that were sent into the channel and that this Receiver has yet to receive.
        // If the returned value from len is larger than the next largest power of 2 of the capacity of the channel
        // any call to recv will return an Err(RecvError::Lagged) and any call to try_recv will return an Err(TryRecvError::Lagged), e.g. if the capacity of the channel is 10, recv will start to return Err(RecvError::Lagged) once len returns values larger than 16.
        methods.add_method("len", |_lua, this, _: ()| {
            let reciever = this.reciever.try_borrow().map_err(|e| {
                LuaError::RuntimeError(format!(
                    "{}. Note that a Reciever cannot recieve concurrently",
                    e
                ))
            })?;

            Ok(reciever.len())
        });

        // Re-subscribes to the channel starting from the current tail element.
        //
        // This Receiver handle will receive a clone of all values sent after it has resubscribed. This will not include elements that are in the queue of the current receiver. Consider the following example.
        methods.add_method("resubscribe", |_lua, this, _: ()| {
            let reciever = this.reciever.try_borrow().map_err(|e| {
                LuaError::RuntimeError(format!(
                    "{}. Note that a Reciever cannot recieve concurrently",
                    e
                ))
            })?;

            let rx = reciever.resubscribe();

            Ok(LuaChannelReciever {
                reciever: Rc::new(RefCell::new(rx)),
            })
        });

        // Returns true if receivers belong to the same channel.
        methods.add_method(
            "same_channel",
            |_lua, this, other: LuaUserDataRef<LuaChannelReciever>| {
                let reciever = this.reciever.try_borrow().map_err(|e| {
                    LuaError::RuntimeError(format!(
                        "{} [this_reciever]. Note that a Reciever cannot recieve concurrently",
                        e
                    ))
                })?;

                let other_reciever = other.reciever.try_borrow().map_err(|e| {
                    LuaError::RuntimeError(format!(
                        "{} [other_reciever]. Note that a Reciever cannot recieve concurrently",
                        e
                    ))
                })?;

                Ok(reciever.same_channel(&other_reciever))
            },
        );

        // Attempts to return a pending value on this receiver without awaiting.
        //
        // This is useful for a flavor of “optimistic check” before deciding to await on a receiver.
        //
        // Compared with recv, this function has three failure cases instead of two (one for closed, one for an empty buffer, one for a lagging receiver).
        //
        // Err(TryRecvError::Closed) is returned when all Sender halves have dropped, indicating that no further values can be sent on the channel.
        //
        // If the Receiver handle falls behind, once the channel is full, newly sent values will overwrite old values. At this point, a call to recv will return with Err(TryRecvError::Lagged) and the Receiver’s internal cursor is updated to point to the oldest value still held by the channel. A subsequent call to try_recv will return this value unless it has been since overwritten. If there are no values to receive, Err(TryRecvError::Empty) is returned.
        methods.add_method("try_recv", |_lua, this, _: ()| {
            let mut reciever = this.reciever.try_borrow_mut().map_err(|e| {
                LuaError::RuntimeError(format!(
                    "{}. Note that a Reciever cannot recieve concurrently",
                    e
                ))
            })?;

            reciever
                .try_recv()
                .map_err(|e| LuaError::external(e.to_string()))
        });
    }
}

pub fn plugin_docs() -> crate::doclib::Plugin {
    crate::doclib::Plugin::default()
        .name("@antiraid/channel")
        .description("Lua channels. Send and recieve messages between Lua threads.")
        .type_mut(
            "LuaChannelSender",
            "LuaChannelSender<T> is the sender part of a channel.",
            |t| {
                t
                .add_generic("T")
                .method_mut("send", |m| {
                    m.description("Sends a value to the channel.")
                    .parameter("data", |p| {
                        p.typ("T").description("The value to send.")
                    })
                    .is_promise(true)
                })
                .method_mut("is_empty", |m| {
                    m.description("Returns true if no queued values.")
                })
                .method_mut("len", |m| {
                    m.description("Returns the number of queued values.")
                })
                .method_mut("reciever_count", |m| {
                    m.description("Returns the number of recievers.")
                })
                .method_mut("same_channel", |m| {
                    m.description("Returns true if two LuaChannelSender instances belong to the same channel.")
                    .parameter("other", |p| {
                        p.typ("LuaChannelSender<T>").description("The other sender to compare.")
                    })
                })
            }
        )
        .type_mut("LuaChannelReciever", "A LuaChannelReciever<T> is the reciever part of a channel.", |t| {
            t
            .add_generic("T")
            .method_mut("recv", |m| {
                m.description("Returns the next value in the channel. Note that a reciever cannot recieve concurrently.")
                .parameter("timeout_secs", |p| {
                    p.typ("u64").description("The number of seconds to wait for a value.")
                })
                .is_promise(true)
            })
            .method_mut("is_empty", |m| {
                m.description("Returns true if there aren’t any messages in the channel that the Receiver has yet to receive.")
            })
            .method_mut("len", |m| {
                m.description("Returns the number of messages that were sent into the channel and that this Receiver has yet to receive.")
            })
            .method_mut("resubscribe", |m| {
                m.description("Re-subscribes to the channel starting from the current tail element.")
            })
            .method_mut("same_channel", |m| {
                m.description("Returns true if receivers belong to the same channel.")
                .parameter("other", |p| {
                    p.typ("LuaChannelReciever<T>").description("The other reciever to compare.")
                })
            })
            .method_mut("try_recv", |m| {
                m.description("Attempts to return a pending value on this receiver without awaiting.")
            })
        })
        .method_mut("new", |m| {
            m.description("Returns the next value in the stream. Note that this is the only function other than `promise.yield` that yields.")
            .parameter("stream", |p| {
                p.typ("LuaStream<T>").description("The stream to get the next value from.")
            })
            .return_("T", |r| {
                r.typ("T").description("LuaChannelReciever<T> is the sender part of a channel")
            })
        })
}

pub fn init_plugin(lua: &Lua) -> LuaResult<LuaTable> {
    let module = lua.create_table()?;

    module.set(
        "new",
        lua.create_function(|_lua, buffer: Option<usize>| {
            let buffer = buffer.unwrap_or(0);

            if buffer > 1024 {
                return Err(LuaError::external(
                    "Buffer size cannot be greater than 1024",
                ));
            }

            if buffer == 0 {
                return Err(LuaError::external("Buffer size cannot be 0"));
            }

            let (sender, recv) = tokio::sync::broadcast::channel(buffer);

            let mv = (
                LuaChannelSender { sender },
                LuaChannelReciever {
                    reciever: Rc::new(RefCell::new(recv)),
                },
            );

            Ok(mv)
        })?,
    )?;

    module.set_readonly(true); // Block any attempt to modify this table

    Ok(module)
}
