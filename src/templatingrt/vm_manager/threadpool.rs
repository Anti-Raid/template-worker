use super::sharedguild::SharedGuild;
use super::threadentry::ThreadEntry;
use crate::config::VmDistributionStrategy;
use crate::templatingrt::state::CreateGuildState;
use crate::templatingrt::vm_manager::ThreadRequest;
use serenity::all::GuildId;
use std::sync::RwLock as StdRwLock;
use tokio::sync::mpsc::UnboundedSender;

pub const DEFAULT_MAX_THREADS: usize = 100; // Maximum number of threads in the pool

pub(super) struct ThreadPool {
    /// The worker threads in the pool
    ///
    /// We can't use a binary heap here due to interior mutability of ordering [count]
    threads: StdRwLock<Vec<ThreadEntry>>,

    /// 2 way thread entry guild map
    sg: SharedGuild,

    /// The maximum number of threads in the pool
    max_threads: usize,
}

impl ThreadPool {
    /// Creates a new thread pool
    pub(super) fn new() -> Self {
        Self {
            threads: StdRwLock::new(Vec::new()),
            sg: SharedGuild::new(),
            max_threads: DEFAULT_MAX_THREADS,
        }
    }

    pub(super) fn send_request<K>(
        &self,
        req: impl Fn(&ThreadEntry) -> Option<(K, ThreadRequest)>,
    ) -> Result<Vec<K>, crate::Error> {
        {
            let threads = self
                .threads
                .try_read()
                .map_err(|_| "Failed to read threads")?;
            let mut data = Vec::with_capacity(threads.len());
            for thread in threads.iter() {
                if let Some((k, treq)) = (req)(&thread) {
                    thread.handle().send(treq)?;
                    data.push(k);
                }
            }

            Ok(data)
        }
    }

    /// Remove broken threads from the pool
    pub(super) async fn remove_unused_threads(&self) -> Result<Vec<u64>, crate::Error> {
        let (mut good_threads, old_threads) = {
            let mut threads = self
                .threads
                .try_write()
                .map_err(|_| "Failed to write lock threads")?;

            let good_threads = Vec::with_capacity(threads.len());
            let old_threads = std::mem::take(&mut *threads);

            (good_threads, old_threads)
        };

        let mut unused = vec![];
        for thread in old_threads {
            // Send Ping to thread
            let (tx, rx) = tokio::sync::oneshot::channel();
            let _ = thread.handle().send(ThreadRequest::RemoveIfUnused { tx });
            tokio::select! {
                resp = rx => {
                    // If we get a response of false, the thread is alive
                    if let Ok(res) = resp {
                        if !res {
                            good_threads.push(thread);
                        }

                        // Not alive
                        continue;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {
                    // Timeout
                }
            };

            // Delete the thread by doing nothing
            log::warn!(
                "Thread {} is unused, removing it from the pool",
                thread.id()
            );
            unused.push(thread.id());
        }

        {
            let mut threads = self
                .threads
                .try_write()
                .map_err(|_| "Failed to write lock threads")?;
            *threads = good_threads;
        }

        Ok(unused)
    }

    /// Adds a new thread to the pool
    pub(super) fn add_thread(&self, cgs: CreateGuildState) -> Result<(), crate::Error> {
        let mut threads = self
            .threads
            .try_write()
            .map_err(|_| "Failed to write lock threads")?;
        threads.push(ThreadEntry::create(cgs, self.sg.clone())?);
        Ok(())
    }

    /// Close a thread in the pool
    pub(super) async fn close_thread(&self, id: u64) -> Result<(), crate::Error> {
        let mut got_entry = None;
        {
            let threads: std::sync::RwLockReadGuard<'_, Vec<ThreadEntry>> = self
                .threads
                .try_read()
                .map_err(|_| "Failed to read lock threads for close_thread")?;

            for th in threads.iter() {
                if th.id() == id {
                    got_entry = Some(th.clone());
                }
            }
        }

        if let Some(th) = got_entry {
            let (tx, rx) = tokio::sync::oneshot::channel();
            th.handle().send(ThreadRequest::CloseThread { tx: Some(tx) })?;
            let r = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                rx,
            ).await;

            // Remove thread from pool
            self.remove_thread(id)?;

            match r {
                Ok(Err(r)) => {
                    return Err(format!(
                        "Failed to close thread due to an error recieving data: {}",
                        r
                    ).into());
                },
                Ok(Ok(_)) => {
                    return Ok(())
                },
                Err(_) => {
                    return Err(
                        "Failed to close thread due to timeout".into()
                    );
                }
            }
        }

        Err("Thread not found".into())
    }

    /// Removes a thread from the pool. This also removes all guild vms attached to said thread as well
    pub(super) fn remove_thread(&self, id: u64) -> Result<(), crate::Error> {
        let idx = {
            let threads = self
                .threads
                .try_read()
                .map_err(|_| "Failed to read lock threads")?;

            let mut idx = None;
            for (i, th) in threads.iter().enumerate() {
                if th.id() == id {
                    idx = Some(i);
                    self.sg.remove_thread_entry(th)?;
                    break;
                }
            }

            idx
        };

        let Some(idx) = idx else {
            return Ok(());
        };

        {
            let mut threads = self
                .threads
                .try_write()
                .map_err(|_| "Failed to write lock threads")?;
            threads.remove(idx);
        }

        Ok(())
    }

    /// Returns the number of threads in the pool
    pub(super) fn threads_len(&self) -> Result<usize, crate::Error> {
        Ok(self
            .threads
            .try_read()
            .map_err(|_| "Failed to read lock threads for threads_len")?
            .len())
    }

    /// Adds a guild to the pool if it does not already exist in the pool
    ///
    /// If the guild already exists in the pool, return the handle
    pub(super) async fn get_guild(
        &self,
        guild: GuildId,
        cgs: CreateGuildState,
    ) -> Result<UnboundedSender<ThreadRequest>, crate::Error> {
        // Check if the guild exists first
        if let Some(handle) = self.sg.get_handle(guild)? {
            return Ok(handle);
        }

        // Flush out threads that have crashed
        let mut broken_th = Vec::new();
        {
            let th = self
                .threads
                .try_read()
                .map_err(|_| "Failed to read lock threads for get_guild")?;

            for th in th.iter() {
                if th.handle().is_closed() {
                    broken_th.push(th.id());
                }
            }
        }

        // Remove broken threads from the pool
        for id in broken_th {
            log::warn!("Removing broken thread with id: {}", id);
            self.remove_thread(id)?;
        }

        if self.threads_len()? < self.max_threads
            || crate::CMD_ARGS.vm_distribution_strategy == VmDistributionStrategy::ThreadPerGuild
        {
            // Add a new thread to the pool
            self.add_thread(cgs)?;
        }

        // Find the thread with the least amount of guilds, then add guild to it
        //
        // This is a simple strategy to balance the load across threads
        let mut min_thread = None;
        let mut min_count = usize::MAX;

        let threads = self
            .threads
            .try_read()
            .map_err(|_| "Could not lock threads")?;
        for thread in threads.iter() {
            let count = thread.server_count();

            if count < min_count {
                min_count = count;
                min_thread = Some(thread);
            }
        }

        let Some(thread) = min_thread else {
            return Err("Failed to add guild to VM pool [no threads]".into());
        };

        // Block out the guild
        self.sg.add_guild(guild, thread.clone())?;

        return Ok(thread.handle().clone());
    }

    pub(super) fn get_guild_if_exists(
        &self,
        guild: GuildId,
    ) -> Result<Option<UnboundedSender<ThreadRequest>>, crate::Error> {
        if let Some(handle) = self.sg.get_handle(guild)? {
            return Ok(Some(handle));
        }

        Ok(None)
    }
}
