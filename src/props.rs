use std::sync::Arc;

use arc_swap::ArcSwap;
use silverpelt::Error;

/// Props
pub struct Props {
    pub cache: ArcSwap<Option<Arc<serenity::all::Cache>>>,
    pub shard_manager: ArcSwap<Option<Arc<serenity::all::ShardManager>>>,
}

#[async_trait::async_trait]
impl silverpelt::data::Props for Props {
    /// Converts the props to std::any::Any
    fn as_any(&self) -> &(dyn std::any::Any + Send + Sync) {
        self
    }

    fn extra_description(&self) -> String {
        "templateWorker".to_string()
    }

    async fn shards(&self) -> Result<Vec<u16>, Error> {
        if let Some(shard_manager) = self.shard_manager.load().as_ref() {
            let mut shards = Vec::new();

            for (id, _) in shard_manager.runners.lock().await.iter() {
                shards.push(id.0);
            }

            Ok(shards)
        } else {
            Ok(Vec::new())
        }
    }

    async fn shard_count(&self) -> Result<u16, Error> {
        let guard = self.cache.load();

        if let Some(cache) = guard.as_ref() {
            Ok(cache.shard_count().get())
        } else {
            Ok(1)
        }
    }

    /// Returns the shard messenger given the shard id
    async fn shard_messenger(
        &self,
        shard_id: serenity::all::ShardId,
    ) -> Result<serenity::all::ShardMessenger, Error> {
        if let Some(shard_manager) = self.shard_manager.load().as_ref() {
            let runners = shard_manager.runners.lock().await;
            let runner = runners
                .get(&shard_id)
                .ok_or_else(|| Error::from(format!("Shard {} not found", shard_id)))?;

            Ok(runner.runner_tx.clone())
        } else {
            Err("Shard manager not initialized".into())
        }
    }

    async fn total_guilds(&self) -> Result<u64, Error> {
        let guard = self.cache.load();

        if let Some(cache) = guard.as_ref() {
            Ok(cache.guilds().len() as u64)
        } else {
            Ok(0)
        }
    }

    async fn total_users(&self) -> Result<u64, Error> {
        let guard = self.cache.load();

        if let Some(cache) = guard.as_ref() {
            let mut count = 0;

            for guild in cache.guilds() {
                {
                    let guild = guild.to_guild_cached(cache);

                    if let Some(guild) = guild {
                        count += guild.member_count;
                    }
                }

                tokio::task::yield_now().await;
            }

            Ok(count)
        } else {
            Ok(0)
        }
    }
}
