use std::sync::{Arc, OnceLock};

/// Serenity shard messenger cache
///
/// Used to store shard messengers for each shard
struct ShardMessengerCache {
    manager: Arc<serenity::all::ShardManager>,
    cache: dashmap::DashMap<serenity::all::ShardId, serenity::all::ShardMessenger>,
}

static SHARD_MESSENGERS: OnceLock<ShardMessengerCache> = OnceLock::new();

/// Returns the total number of shards
pub fn shard_count() -> Result<std::num::NonZeroU16, silverpelt::Error> {
    let cache = SHARD_MESSENGERS
        .get()
        .ok_or("Shard messenger cache not initialized")?;

    let shard_count =
        std::num::NonZeroU16::new(cache.cache.len().try_into()?).ok_or("No shards available")?;
    Ok(shard_count)
}

/// Returns the shard ids available
pub fn shard_ids() -> Result<Vec<serenity::all::ShardId>, silverpelt::Error> {
    let cache = SHARD_MESSENGERS
        .get()
        .ok_or("Shard messenger cache not initialized")?;

    let mut shard_ids = Vec::new();

    for refmut in cache.cache.iter() {
        shard_ids.push(*refmut.key());
    }

    Ok(shard_ids)
}

#[allow(dead_code)]
/// Get the shard messenger for a guild
pub fn shard_messenger_for_guild(
    guild_id: serenity::all::GuildId,
) -> Result<serenity::all::ShardMessenger, silverpelt::Error> {
    let cache = SHARD_MESSENGERS
        .get()
        .ok_or("Shard messenger cache not initialized")?;

    let guild_shard_count =
        std::num::NonZeroU16::new(cache.cache.len().try_into()?).ok_or("No shards available")?;
    let guild_shard_id = serenity::all::utils::shard_id(guild_id, guild_shard_count);
    let guild_shard_id = serenity::all::ShardId(guild_shard_id);

    if let Some(shard) = cache.cache.get(&guild_shard_id) {
        return Ok(shard.value().clone());
    }

    Err("Shard not found".into())
}

/// Sets up the shard manager given client
pub async fn setup_shard_messenger(client: &serenity::all::Client) {
    let guard = client.shard_manager.runners.lock().await;
    let cache = dashmap::DashMap::new();

    for (shard_id, runner_info) in guard.iter() {
        cache.insert(*shard_id, runner_info.runner_tx.clone());
    }

    let shard_manager = client.shard_manager.clone();
    SHARD_MESSENGERS.get_or_init(|| ShardMessengerCache {
        cache,
        manager: shard_manager,
    });
}

pub async fn update_shard_messengers() {
    let sm = SHARD_MESSENGERS
        .get()
        .expect("Shard messenger cache not initialized");
    let guard = sm.manager.runners.lock().await;

    sm.cache.clear();
    for (shard_id, runner_info) in guard.iter() {
        sm.cache.insert(*shard_id, runner_info.runner_tx.clone());
    }
}
