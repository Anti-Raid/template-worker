use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver, unbounded_channel};
use tokio_util::time::DelayQueue;
use std::rc::Rc;
use std::cell::RefCell;
use std::time::Duration;
use crate::worker::workerstate::WorkerState;

use super::workervmmanager::Id;
use super::workerstate::KeyExpiry;
use futures::StreamExt;

use super::workerfilter::WorkerFilter;

// The maximum duration of a delay before we need to do some tricks
// From https://docs.rs/tokio-util/latest/src/tokio_util/time/wheel/mod.rs.html 
const NUM_LEVELS: usize = 6;
const MAX_DURATION: u64 = (1 << (6 * NUM_LEVELS)) - 1;
const MAX_DURATION_OBJ: Duration = Duration::from_millis(MAX_DURATION-5000);

enum KeyExpiryChannelMessage {
    Repopulate, // Repopulate the entire key expiry channel
}

/// A channel to send out key expiry events when needed
#[derive(Clone)]
pub struct KeyExpiryChannel {
    state: WorkerState,
    filter: WorkerFilter,
    tx: UnboundedSender<KeyExpiryChannelMessage>,
    sink: Rc<RefCell<Option<UnboundedSender<(Id, KeyExpiry)>>>>
}

impl KeyExpiryChannel {
    /// Create a new key expiry channel
    pub fn new(state: WorkerState, filter: WorkerFilter) -> Self {
        let (tx, rx): (UnboundedSender<KeyExpiryChannelMessage>, UnboundedReceiver<KeyExpiryChannelMessage>) = unbounded_channel();

        let chan = Self { state, tx, filter, sink: Rc::default() };

        // Run the channel in a separate task
        let chan_ref = chan.clone();
        tokio::task::spawn_local(async move {
            chan_ref.run(rx).await;
        });

        chan
    }

    async fn run(&self, mut rx: UnboundedReceiver<KeyExpiryChannelMessage>) {
        let mut delay_queue = self.create_queue().await.unwrap_or_else(|e| {
            log::error!("FATAL: Failed to create key expiry delay queue: {}", e);
            std::process::exit(1);
        });

        loop {
            tokio::select! {
                Some(msg) = rx.recv() => {
                    match msg {
                        KeyExpiryChannelMessage::Repopulate => {
                            log::info!("Repopulating key expiry channel");
                            match self.create_queue().await {
                                Ok(new_queue) => {
                                    delay_queue = new_queue;
                                },
                                Err(e) => {
                                    log::error!("Failed to repopulate key expiry channel: {e}, continuing with old");
                                }
                            }
                        },
                    }
                },
                Some(data) = delay_queue.next() => {
                    let data = data.into_inner();

                    if data.1.expires_at > chrono::Utc::now() {
                        // Not expired yet, reinsert with new duration
                        let dt = Self::get_expiry(&data.1);
                        delay_queue.insert(data, dt);
                        continue; // Skip dispatching as we haven't expired yet
                    }

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
    async fn create_queue(&self) -> Result<DelayQueue<(Id, KeyExpiry)>, crate::Error> {
        let mut delay_queue = DelayQueue::new();
        let expired_keys = self.state.get_key_expiries().await?;
        for data in expired_keys {
            if self.filter.is_allowed(data.0) {
                for expiry in data.1 {
                    let dt = Self::get_expiry(&expiry);
                    delay_queue.insert((data.0, expiry), dt);
                }
            }
        }

        Ok(delay_queue)
    }

    /// Returns the next expiry duration for a key expiry
    /// 
    /// If the key is already expired, returns a random duration between 5 and 10 seconds
    /// to ensure a random fanout
    /// 
    /// If the key is not expired, returns the duration until it expires, capped at MAX_DURATION
    /// to avoid issues with tokio's DelayQueue wheel having a limit on maximum duration
    fn get_expiry(key_expiry: &KeyExpiry) -> Duration {
        // Not expired yet, reinsert with new duration
        let dt = key_expiry.expires_at - chrono::Utc::now();
        let dt_is_neg = dt.num_milliseconds() < 0;
        if !dt_is_neg {
            let mut duration_std = dt.to_std().unwrap();
            if duration_std.as_millis() >= MAX_DURATION_OBJ.as_millis() {
                // We cap it to the MAX_DURATION
                // 
                // when we (somehow) do get the event out,
                // we simply see that we haven't expired yet and repush for more time
                duration_std = MAX_DURATION_OBJ
            }

            log::info!("Reinserting key expiry to delay queue: {:?}", duration_std);

            duration_std
        } else {
            // Create a random duration between 5 and 10 seconds
            let duration_secs = rand::random_range(5..=10);
            let duration = Duration::from_secs(duration_secs);
            duration
        }
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
    tx: UnboundedReceiver<(Id, KeyExpiry)>,
    sink: Rc<RefCell<Option<UnboundedSender<(Id, KeyExpiry)>>>>,
}

impl std::ops::Deref for KeyExpiryChannelSubscriber {
    type Target = UnboundedReceiver<(Id, KeyExpiry)>;

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