use std::{cell::RefCell, rc::Rc};

use khronos_runtime::primitives::event::CreateEvent;

use crate::{mesophyll::{message::MesophyllMessage, cache::TemplateCacheView}, templatedb::{base_template::TemplateReference}, worker::{workerdispatch::DispatchTemplateResult, workervmmanager::Id}};

enum TemplateCacheViewState {
    Empty,
    Ready(TemplateCacheView),
}

#[allow(dead_code)]
impl TemplateCacheViewState {
    pub fn as_view(&self) -> Option<&TemplateCacheView> {
        match self {
            TemplateCacheViewState::Empty => None,
            TemplateCacheViewState::Ready(view) => Some(view),
        }
    }

    pub fn as_view_mut(&mut self) -> Option<&mut TemplateCacheView> {
        match self {
            TemplateCacheViewState::Empty => None,
            TemplateCacheViewState::Ready(view) => Some(view),
        }
    }
}

#[async_trait::async_trait]
pub trait MesophyllClientHandler {
    /// Called when the Mesophyll client is ready with initial state
    async fn ready(&self, client: &MesophyllClient) -> Result<(), crate::Error>;

    /// Called when a dispatched event result is received
    async fn dispatch_result(&self, client: &MesophyllClient, id: Id, event: CreateEvent) -> DispatchTemplateResult;

    /// Called when a scoped dispatched event result is received
    async fn dispatch_scoped_result(&self, client: &MesophyllClient, id: Id, event: CreateEvent, scopes: Vec<String>) -> DispatchTemplateResult;

    /// Called when a cache regeneration is requested
    async fn regenerate_cache(&self, client: &MesophyllClient, id: Id) -> Result<(), crate::Error>;

    /// Called when cache regeneration is requested for multiple template references
    async fn regenerate_caches(&self, client: &MesophyllClient, ids: Vec<TemplateReference>) -> Result<(), crate::Error>;
}

/// Mesophyll client, NOT THREAD SAFE
#[derive(Clone)]
pub struct MesophyllClient {
    template_cache: Rc<RefCell<TemplateCacheViewState>>,
    handler: Rc<dyn MesophyllClientHandler>,
}

#[allow(dead_code)]
impl MesophyllClient {
    pub fn new<T>(handler: T) -> Self 
    where
        T: MesophyllClientHandler + 'static,
    {
        Self {
            template_cache: Rc::new(RefCell::new(TemplateCacheViewState::Empty)),
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
            MesophyllMessage::Ready { templates } => {
                // This is safe as the whole thing is single threaded
                *self.template_cache.borrow_mut() = TemplateCacheViewState::Ready(templates);
                log::info!("Recieved new template cache from Mesophyll");
                self.handler.ready(self).await?;
            },
            MesophyllMessage::TemplateUpdate { update } => {
                if let Some(view) = self.template_cache.borrow_mut().as_view_mut() {
                    log::info!("Recieved template update from Mesophyll and applying to cache");
                   if let Some(refs) = view.apply_cache_update(update) {
                        self.handler.regenerate_caches(self, refs).await?;
                   }
                } else {
                    log::warn!("Recieved template update from Mesophyll but template cache is not ready");
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
            MesophyllMessage::RegenerateCache { id, req_id } => {
                match self.handler.regenerate_cache(self, id).await {
                    Ok(_) => {},
                    Err(e) => {
                        let response = MesophyllMessage::ResponseError { error: format!("{e}"), req_id };
                        let _ = self.send_message(response);
                        return Err(e);
                    }
                };
                let response = MesophyllMessage::ResponseAck { req_id };
                let _ = self.send_message(response);
            }
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