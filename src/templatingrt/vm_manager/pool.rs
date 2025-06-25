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
pub enum PoolBacker {
    ThreadPool(ThreadPool),
}

/// Abstraction around the current pool implementation being used
pub struct Pool {
    inner: PoolBacker,
}

impl Pool {
    /// Creates a new pool with the specified inner pool
    pub fn new(backer: PoolBacker) -> Self {
        Self { inner: backer }
    }

    /// Creates a new pool backed by a thread pool distribution
    pub fn new_threadpool() -> Self {
        Self::new(PoolBacker::ThreadPool(ThreadPool::new()))
    }

    /// Returns the number of worker process
    pub fn len(&self) -> Result<usize, silverpelt::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => tp.threads_len(),
        }
    }

    /// Gets a guild from the pool
    pub async fn get_guild(
        &self,
        guild: GuildId,
        cgs: CreateGuildState,
    ) -> Result<UnboundedSender<ThreadRequest>, silverpelt::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => tp.get_guild(guild, cgs).await,
        }
    }

    /// Gets a guild from the pool if it exists right now
    pub fn get_guild_if_exists(
        &self,
        guild: GuildId,
    ) -> Result<Option<UnboundedSender<ThreadRequest>>, silverpelt::Error> {
        match &self.inner {
            PoolBacker::ThreadPool(tp) => tp.get_guild_if_exists(guild),
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
