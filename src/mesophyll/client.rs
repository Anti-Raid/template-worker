use std::{cell::RefCell, rc::Rc};

use khronos_runtime::primitives::event::CreateEvent;

use crate::{mesophyll::{message::MesophyllMessage, cache::TemplateCacheView}, worker::{workerdispatch::DispatchTemplateResult, workervmmanager::Id}};

#[async_trait::async_trait]
pub trait MesophyllClientHandler {
    /// Called when the Mesophyll client is ready with initial state
    async fn ready(&self, client: &MesophyllClient) -> Result<(), crate::Error>;

    /// Called when a dispatched event result is received
    async fn dispatch_result(&self, client: &MesophyllClient, id: Id, event: CreateEvent) -> DispatchTemplateResult;

    /// Called when a scoped dispatched event result is received
    async fn dispatch_scoped_result(&self, client: &MesophyllClient, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult;
}

/// Mesophyll client, NOT THREAD SAFE
#[derive(Clone)]
pub struct MesophyllClient {
    template_cache: Rc<RefCell<TemplateCacheView>>,
    handler: Rc<dyn MesophyllClientHandler>,
}

#[allow(dead_code)]
impl MesophyllClient {
    pub fn new<T>(handler: T) -> Self 
    where
        T: MesophyllClientHandler + 'static,
    {
        Self {
            template_cache: Rc::new(RefCell::new(TemplateCacheView::new())),
            handler: Rc::new(handler),
        }
    }

    pub fn send_message(&self, _message: MesophyllMessage) -> Result<(), crate::Error> {
        todo!();
    }

    pub async fn process_message(&self, event: MesophyllMessage) -> Result<(), crate::Error> {
        match event {
            MesophyllMessage::Identify { id: _, session_key: _ } => {
                // Nothing client can do.
            },
            MesophyllMessage::Ready {} => {
                self.handler.ready(self).await?;
            },
            MesophyllMessage::TemplateCacheUpdate { update, req_id } => {
                if let Ok(mut view) = self.template_cache.try_borrow_mut() {
                    log::info!("Recieved template update from Mesophyll and applying to cache");
                    view.apply_cache_update(update);
                    self.send_message(MesophyllMessage::ResponseAck { req_id })?;
                } else {
                    log::warn!("Recieved template update from Mesophyll but template cache is not ready");
                    self.send_message(MesophyllMessage::ResponseError { error: "Template cache not ready".to_string(), req_id })?;
                    return Err("Template cache not ready".into());
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
            }
            MesophyllMessage::ResponseAck { .. } => {
                // Nothing client can do.
            }
            MesophyllMessage::ResponseError { .. } => {
                // Nothing client can do.
            }
        }

        Ok(())
    }
}