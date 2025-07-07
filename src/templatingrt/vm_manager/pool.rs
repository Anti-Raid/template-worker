use super::threadentry::ThreadRequest;
use super::threadentry::{ThreadClearInactiveGuilds, ThreadMetrics};
use super::threadpool::ThreadPool;
use crate::templatingrt::CreateGuildState;
use futures::stream::FuturesUnordered;
use futures::FutureExt;
use futures::StreamExt;
use serenity::all::GuildId;
use std::sync::LazyLock;
use tokio::sync::mpsc::UnboundedSender;

// TODO: Make this not global state
pub static POOL: LazyLock<Pool> = LazyLock::new(Pool::new_threadpool);

/// Inner backer of the pool
/// 
/// This is an internal structure used to allow for different pool implementations in the future
/// without changing the public API of the `Pool` struct.
pub(super) enum PoolBacker {
    ThreadPool(ThreadPool),
}

/// Abstraction around the current pool implementation being used
pub struct Pool {
    inner: PoolBacker,
}

impl Pool {
    /// Creates a new pool backed by a thread pool distribution
    pub fn new_threadpool() -> Self {
        Self { inner: PoolBacker::ThreadPool(ThreadPool::new()) }
    }

    /// Returns the number of worker process
    pub fn len(&self) -> Result<usize, crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => tp.threads_len(),
        }
    }

    /// Gets a guild from the pool
    pub async fn get_guild(
        &self,
        guild: GuildId,
        cgs: CreateGuildState,
    ) -> Result<UnboundedSender<ThreadRequest>, crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => tp.get_guild(guild, cgs).await,
        }
    }

    /// Gets a guild from the pool if it exists right now
    pub fn get_guild_if_exists(
        &self,
        guild: GuildId,
    ) -> Result<Option<UnboundedSender<ThreadRequest>>, crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => tp.get_guild_if_exists(guild),
        }
    }

    /// Ping all threads returning a list of threads which responded
    pub async fn ping(
        &self,
    ) -> Result<Vec<u64>, crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => {
                let fu = FuturesUnordered::new();
                let futs = tp.send_request(|te| {
                    let (tx, rx) = tokio::sync::oneshot::channel();

                    let tid = te.id();
                    Some((
                        rx.map(move |_x| tid),
                        ThreadRequest::Ping { tx },
                    ))
                })?;

                for fut in futs {
                    fu.push(fut);
                }

                let resp = fu.collect().await;

                Ok(resp)
            }
        }
    }

    /// Remove inactive guilds
    pub async fn clear_inactive_guilds(
        &self,
    ) -> Result<Vec<ThreadClearInactiveGuilds>, crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => {
                let fu = FuturesUnordered::new();
                let futs = tp.send_request(|te| {
                    let (tx, rx) = tokio::sync::oneshot::channel();

                    let tid = te.id();
                    Some((
                        rx.map(move |x| ThreadClearInactiveGuilds {
                            tid,
                            cleared: x.unwrap_or_default(),
                        }),
                        ThreadRequest::ClearInactiveGuilds { tx },
                    ))
                })?;

                for fut in futs {
                    fu.push(fut);
                }

                let resp = fu.collect().await;

                Ok(resp)
            }
        }
    }

    /// Remove unused threads from the pool
    pub async fn remove_unused_threads(&self) -> Result<Vec<u64>, crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => tp.remove_unused_threads().await,
        }
    }

    /// Closes a thread in the pool
    pub async fn close_thread(&self, id: u64) -> Result<(), crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => tp.close_thread(id).await,
        }
    }

    /// Get VM metrics for all
    pub async fn get_vm_metrics_for_all(&self) -> Result<Vec<ThreadMetrics>, crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => {
                let fu = FuturesUnordered::new();
                let futs = tp.send_request(|te| {
                    let (tx, rx) = tokio::sync::oneshot::channel();

                    let tid = te.id();
                    Some((
                        rx.map(move |x| ThreadMetrics {
                            tid,
                            vm_metrics: x.unwrap_or_default(),
                        }),
                        ThreadRequest::GetVmMetrics { tx },
                    ))
                })?;

                for fut in futs {
                    fu.push(fut);
                }

                let resp = fu.collect().await;

                Ok(resp)
            }
        }
    }

    /// Get VM metrics for all
    pub async fn get_vm_metrics_by_tid(
        &self,
        o_tid: u64,
    ) -> Result<Vec<ThreadMetrics>, crate::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => {
                let fu = FuturesUnordered::new();
                let futs = tp.send_request(move |te| {
                    if o_tid != te.id() {
                        return None;
                    }

                    let (tx, rx) = tokio::sync::oneshot::channel();

                    let tid = te.id();
                    Some((
                        rx.map(move |x| ThreadMetrics {
                            tid,
                            vm_metrics: x.unwrap_or_default(),
                        }),
                        ThreadRequest::GetVmMetrics { tx },
                    ))
                })?;

                for fut in futs {
                    fu.push(fut);
                }

                let resp = fu.collect().await;

                Ok(resp)
            }
        }
    }
}
