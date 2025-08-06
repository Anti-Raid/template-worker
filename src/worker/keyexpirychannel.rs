use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded_channel};
use tokio_util::time::DelayQueue;
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::Arc;
use std::time::Duration;
use super::workervmmanager::Id;
use super::workercachedata::WorkerCacheData;
use super::workerdb::KeyExpiry;
use futures::StreamExt;

use super::workerfilter::WorkerFilter;

enum KeyExpiryChannelMessage {
    Repopulate, // Repopulate the entire key expiry channel
}

/// A channel to send out key expiry events when needed
#[derive(Clone)]
pub struct KeyExpiryChannel {
    cache: WorkerCacheData,
    filter: WorkerFilter,
    tx: UnboundedSender<KeyExpiryChannelMessage>,
    sink: Rc<RefCell<Option<UnboundedSender<(Id, Arc<KeyExpiry>)>>>>
}

impl KeyExpiryChannel {
    /// Create a new key expiry channel
    pub fn new(cache: WorkerCacheData, filter: WorkerFilter) -> Self {
        let (tx, rx): (UnboundedSender<KeyExpiryChannelMessage>, UnboundedReceiver<KeyExpiryChannelMessage>) = unbounded_channel();

        let chan = Self { cache, tx, filter, sink: Rc::default() };

        // Run the channel in a separate task
        let chan_ref = chan.clone();
        tokio::task::spawn_local(async move {
            chan_ref.run(rx).await;
        });

        chan
    }

    async fn run(&self, mut rx: UnboundedReceiver<KeyExpiryChannelMessage>) {
        let mut delay_queue = self.create_queue();

        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    match msg {
                        KeyExpiryChannelMessage::Repopulate => {
                            delay_queue = self.create_queue();
                        },
                    }
                },
                Some(data) = delay_queue.next() => {
                    let data = data.into_inner();
                    if self.filter.is_allowed(data.0) {
                        let sink_guard = self.sink.borrow();
                        if let Some(sink) = sink_guard.as_ref() {
                            let _ = sink.send(data); // Dispatch if needed
                        }
                    }
                }
            }
        }
    }

    /// Populates the key expiry channel with current data from WorkerCacheData
    fn create_queue(&self) -> DelayQueue<(Id, Arc<KeyExpiry>)> {
        let mut delay_queue = DelayQueue::new();
        let expired_keys = self.cache.get_all_key_expiries();
        for data in expired_keys {
            if self.filter.is_allowed(data.0) {
                let dt = data.1.expires_at - chrono::Utc::now();
                let dt_is_neg = dt.num_seconds() < 0;
                if !dt_is_neg {
                    let duration_std = dt.to_std().unwrap();
                    delay_queue.insert(data, duration_std);
                } else {
                    // Create a random duration between 5 and 10 seconds
                    let duration_secs = rand::random_range(5..=10);
                    let duration = Duration::from_secs(duration_secs);
                    delay_queue.insert(data, duration);
                }
            }
        }

        delay_queue
    }

    /// Send a repopulate message to the channel
    pub fn repopulate(&self) -> Result<(), crate::Error> {
        self.tx.send(KeyExpiryChannelMessage::Repopulate).map_err(|e| {
            format!("Failed to send repopulate message to key expiry channel: {}", e).into()
        })
    }

    /// Sets the sink for the key expiry channel
    ///
    /// Note that currently one sink is allowed at a time
    pub fn subscribe(&self) -> Result<KeyExpiryChannelSubscriber, crate::Error> {
        if self.sink.borrow().is_some() {
            return Err("Key expiry channel already has a sink".into());
        }

        let (tx, rx) = unbounded_channel();
        self.sink.replace(Some(tx));

        Ok(KeyExpiryChannelSubscriber {
            tx: rx,
            sink: self.sink.clone(),
        })
    }
}

/// A subscriber for the key expiry channel
/// 
/// This automatically drops the sink when the subscriber is dropped
pub struct KeyExpiryChannelSubscriber {
    tx: UnboundedReceiver<(Id, Arc<KeyExpiry>)>,
    sink: Rc<RefCell<Option<UnboundedSender<(Id, Arc<KeyExpiry>)>>>>,
}

impl std::ops::Deref for KeyExpiryChannelSubscriber {
    type Target = UnboundedReceiver<(Id, Arc<KeyExpiry>)>;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl std::ops::DerefMut for KeyExpiryChannelSubscriber {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tx
    }
}

impl Drop for KeyExpiryChannelSubscriber {
    fn drop(&mut self) {
        self.sink.replace(None); // Clear the sink when dropped
    }
}