use std::rc::Rc;

use crate::dispatch::parse_event;
use crate::events::{AntiraidEvent, KeyExpiryEvent};

use super::workervmmanager::Id;
use super::workerdispatch::WorkerDispatch;
use super::keyexpirychannel::KeyExpiryChannel;

/// Inner non-cloneable structure for WorkerKeyExpiry
/// 
/// Avoids heavy cloning during the key expiry handling process
pub struct KeyExpiryInner {
    /// Worker Key Expiry Events
    key_expiry_chan: KeyExpiryChannel,
    /// Worker Event Dispatch
    dispatch: WorkerDispatch,
}

#[derive(Clone)]
pub struct WorkerKeyExpiry {
    /// Inner structure that contains the actual data
    /// 
    /// Wrapped in single Rc for shallow cloning to avoid refcounting 100s of different internal
    /// structures
    inner: Rc<KeyExpiryInner>,
}

impl std::ops::Deref for WorkerKeyExpiry {
    type Target = KeyExpiryInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl WorkerKeyExpiry {
    pub fn new(dispatch: WorkerDispatch, key_expiry_chan: KeyExpiryChannel) -> Self {
        let expirer = Self { 
            inner: Rc::new(KeyExpiryInner {
                dispatch, 
                key_expiry_chan, 
            })
        };

        // Run the key expiry task
        let expirer_ref = expirer.clone();
        tokio::task::spawn_local(async move {
            expirer_ref.run().await;
        });

        expirer
    }

    /// Remove a key expiry from the database and cache
    /// 
    /// NOTE: This does not repopulate the key expiry channel as it is not needed in the cases this is called
    async fn remove_key_expiry(&self, id: Id, kv_id: &str) -> Result<(), crate::Error> {
        self.dispatch.worker_state().remove_key_expiry(id, kv_id).await?;
        Ok(())
    }

    
    async fn run(&self) {
        let mut subscriber = self.key_expiry_chan.subscribe().expect("Failed to subscribe to key expiry channel");

        let mut set = tokio::task::JoinSet::new();

        // Note that KeyExpiryChannel will apply filtering and only send events for tenants
        // that pass the filter
        while let Some((id, data)) = subscriber.recv().await {
            let event = AntiraidEvent::KeyExpiry(KeyExpiryEvent {
                id: data.id.clone(),
                key: data.key.clone(),
                scopes: data.scopes.clone(),
            });

            let Ok(create_event) = parse_event(&event) else {
                log::error!("Failed to parse key expiry event: {:?}", event);
                continue;
            };

            let self_ref = self.clone();
            set.spawn_local(async move {
                if let Err(e) = self_ref.dispatch.dispatch_event(id, create_event).await {
                    log::error!("Error in key expiry: {:?}", e);
                }

                match self_ref.remove_key_expiry(id, &data.id).await {
                    Ok(_) => {}
                    Err(e) => {
                        log::error!("Error removing key expiry: {e:?}");
                    }
                }
            });
        }
    }
}
