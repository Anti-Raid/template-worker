use std::rc::Rc;

use khronos_runtime::primitives::event::CreateEvent;
use tokio::sync::Notify;

use crate::worker::workerdispatch::WorkerDispatch;

/// Mesophyll client, NOT THREAD SAFE
#[derive(Clone)]
pub struct MesophyllClient {
    dispatch: WorkerDispatch,
}

#[allow(dead_code)]
impl MesophyllClient {
    pub fn new(dispatch: WorkerDispatch) -> Self {
        Self {
            dispatch,
        }
    }

    pub fn send_message(&self, _message: MesophyllMessage) -> Result<(), crate::Error> {
        todo!();
    }

    pub async fn process_message(&self, event: MesophyllMessage) -> Result<(), crate::Error> {
        match event {
            MesophyllMessage::Identify { id: _, session_key: _ } => {
                // Nothing client can do.
                log::warn!("Mesophyll client received unexpected Identify message");
            },
            MesophyllMessage::Ready {} => {
                log::info!("Mesophyll client is now ready");
                self.ready.notify_waiters();
                self.handler.ready(self).await?;
            },
            MesophyllMessage::Relay { msg, req_id } => {
                log::info!("Mesophyll client received relay message");
                match self.handler.relay(self, msg).await {
                    Ok(_) => {
                        self.send_message(MesophyllMessage::ResponseAck { req_id })?;
                    },
                    Err(e) => {
                        log::error!("Error handling relay message: {}", e);
                        self.send_message(MesophyllMessage::ResponseError { error: format!("Error handling relay message: {}", e), req_id })?;
                        return Err(e);  
                    }
                }
            }
            MesophyllMessage::DispatchEvent { id, event, req_id } => {
                let result = self.handler.dispatch_result(self, id, event).await;
                let response = MesophyllMessage::ResponseDispatchResult { result: result.into(), req_id };
                self.send_message(response)?;
            }
            MesophyllMessage::DispatchScopedEvent { id, event, scopes, req_id } => {
                let result = self.handler.dispatch_scoped_result(self, id, event, scopes).await;
                let response = MesophyllMessage::ResponseDispatchResult { result: result.into(), req_id };
                let _ = self.send_message(response);
            },
            MesophyllMessage::ResponseDispatchResult { .. } => {
                // Nothing client can do.
                log::warn!("Mesophyll client received unexpected ResponseDispatchResult message");
            }
            MesophyllMessage::ResponseAck { .. } => {
                // Nothing client can do.
                log::warn!("Mesophyll client received unexpected ResponseAck message");
            }
            MesophyllMessage::ResponseError { .. } => {
                // Nothing client can do.
                log::warn!("Mesophyll client received unexpected ResponseError message");
            }
        }

        Ok(())
    }
}