use super::threadentry::ThreadRequest;
use crate::templatingrt::vm_manager::ThreadEntry;
use serenity::all::GuildId;
use std::collections::HashMap;
use std::sync::{Arc, RwLock as StdRwLock};
use tokio::sync::mpsc::UnboundedSender;

#[derive(Clone)]
/// A two-way binding between a guild id and its associated worker thread
/// 
/// This allows for efficient tracking of guilds
pub(super) struct SharedGuild {
    /// A record mapping a guild id to the thread its currently on
    guilds: Arc<StdRwLock<HashMap<GuildId, ThreadEntry>>>,

    /// A record mapping a entry to the guilds it is associated with
    entries: Arc<StdRwLock<HashMap<ThreadEntry, Vec<GuildId>>>>,
}

impl SharedGuild {
    pub(super) fn new() -> Self {
        Self {
            guilds: StdRwLock::new(HashMap::new()).into(),
            entries: StdRwLock::new(HashMap::new()).into(),
        }
    }

    pub(super) fn add_guild(
        &self,
        guild_id: GuildId,
        thread_entry: ThreadEntry,
    ) -> Result<(), crate::Error> {
        let mut guilds = self.guilds.try_write().map_err(|e| e.to_string())?;
        let mut entries = self.entries.try_write().map_err(|e| e.to_string())?;

        if let Some(old_entry) = guilds.insert(guild_id, thread_entry.clone()) {
            entries
                .entry(old_entry)
                .or_default()
                .retain(|x| *x != guild_id);
        }

        entries.entry(thread_entry).or_default().push(guild_id);

        Ok(())
    }

    pub(super) fn remove_guild(&self, guild_id: GuildId) -> Result<(), crate::Error> {
        let mut guilds = self.guilds.try_write().map_err(|e| e.to_string())?;

        let Some(thread_entry) = guilds.remove(&guild_id) else {
            return Ok(());
        };

        let mut entries = self.entries.try_write().map_err(|e| e.to_string())?;
        entries
            .entry(thread_entry)
            .or_default()
            .retain(|x| *x != guild_id);

        Ok(())
    }

    pub(super) fn remove_thread_entry(&self, thread_entry: &ThreadEntry) -> Result<(), crate::Error> {
        let mut entries = self.entries.try_write().map_err(|e| e.to_string())?;

        let tid = thread_entry.id();
        let Some(guild_list) = entries.remove(thread_entry) else {
            return Ok(());
        };

        let mut guilds = self.guilds.try_write().map_err(|e| e.to_string())?;

        for guild in guild_list {
            if let Some(te) = guilds.get(&guild) {
                if te.id() != tid {
                    continue;
                }
            }

            guilds.remove(&guild);
        }

        Ok(())
    }

    pub fn get_handle(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<UnboundedSender<ThreadRequest>>, crate::Error> {
        Ok(self
            .guilds
            .try_read()
            .map_err(|e| e.to_string())?
            .get(&guild_id)
            .map(|x| x.handle().clone()))
    }
}
