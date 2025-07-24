use super::template::Template;
use super::{LuaVmAction, RenderTemplateHandle, ThreadRequest, MAX_TEMPLATES_RETURN_WAIT_TIME};
use crate::data::Data;
use crate::dispatch::parse_event;
use crate::templatingrt::template::{DefaultableOverlayFS, TemplatingTypes};
use antiraid_types::ar_event::AntiraidEvent;
use khronos_runtime::primitives::event::CreateEvent;
use moka::future::Cache;
use rust_embed::Embed;
use serenity::all::GuildId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use vfs::FileSystem;

pub const MAX_EXTENDS: i64 = 12; // Maximum number of times a key expiry can be extended due to a failure in its handling

// Test base will be used for builtins in the future

#[allow(dead_code)]
pub fn str_to_fs(s: &str) -> vfs::MemoryFS {
    let fs = vfs::MemoryFS::new();
    fs.create_file("/init.luau")
        .unwrap()
        .write_all(s.as_bytes())
        .unwrap();
    fs
}

/// Builtins
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/../../builtins"]
#[prefix = ""]
pub struct Builtins;

/// Builtins patches
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/../../builtins_patches"]
#[prefix = ""]
pub struct BuiltinsPatches;

// Replace this with the new builtins template once ready to deploy
pub const BUILTINS_NAME: &str = "$builtins";
pub static BUILTINS: LazyLock<Arc<Template>> = LazyLock::new(|| {
    let templ = Template {
        content: DefaultableOverlayFS(vfs::OverlayFS::new(&vec![
            vfs::EmbeddedFS::<BuiltinsPatches>::new().into(),
            vfs::EmbeddedFS::<Builtins>::new().into(),
            vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
        ])),
        name: BUILTINS_NAME.to_string(),
        events: vec![
            "INTERACTION_CREATE".to_string(),
            "KeyExpiry[builtins.remindme]".to_string(),
            "GetSettings".to_string(),
            "ExecuteSetting[guildmembers]".to_string(),
            "ExecuteSetting[guildroles]".to_string(),
            "ExecuteSetting[scripts]".to_string(),
        ],
        allowed_caps: vec!["*".to_string()],
        ..Default::default()
    };

    Arc::new(templ)
});
pub static BUILTINS_ARC_VEC: LazyLock<Arc<Vec<Arc<Template>>>> =
    LazyLock::new(|| Arc::new(vec![BUILTINS.clone()]));
pub const USE_BUILTINS: bool = true;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct KeyExpiry {
    pub id: String,
    pub key: String,
    pub scopes: Vec<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// This should be in descending order of expires_at
pub static KEY_EXPIRIES: LazyLock<Cache<GuildId, Arc<Vec<Arc<KeyExpiry>>>>> =
    LazyLock::new(|| Cache::builder().build());

pub static TEMPLATES_CACHE: LazyLock<Cache<GuildId, Arc<Vec<Arc<Template>>>>> =
    LazyLock::new(|| Cache::builder().build());

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum DeferredCacheRegenMode {
    NoOnReady,
    OnReady {
        modified: Vec<String>,
    },
    FlushMultiple {
        other_guilds: Vec<GuildId>,
        flush_self: bool,
    },
}

// Deferred cache regens
//
// Useful for OPs that need to regenerate the cache after a return from Luau VM
pub static DEFERRED_CACHE_REGENS: LazyLock<Cache<GuildId, DeferredCacheRegenMode>> =
    LazyLock::new(|| Cache::builder().build());

// Regenerates the cache for a guild (and other affected guilds), and dispatches OnStartup events if needed
pub async fn regenerate_deferred(
    context: &serenity::all::Context,
    data: &Data,
    guild_id: GuildId,
) -> Result<(), crate::Error> {
    if let Some(mode) = DEFERRED_CACHE_REGENS.remove(&guild_id).await {
        regenerate_cache(context, data, guild_id).await?;

        match mode {
            DeferredCacheRegenMode::NoOnReady => {}
            DeferredCacheRegenMode::OnReady { modified } => {
                let ce = crate::dispatch::parse_event(&AntiraidEvent::OnStartup(modified))?;
                crate::dispatch::dispatch(context, data, ce, guild_id)
                    .await
                    .map_err(|e| format!("Failed to dispatch OnStartup event: {:?}", e))?;
            }
            DeferredCacheRegenMode::FlushMultiple {
                other_guilds,
                flush_self,
            } => {
                if flush_self {
                    crate::dispatch::dispatch(
                        context, 
                        data, 
                        parse_event(&AntiraidEvent::OnStartup(vec![]))?, 
                        guild_id
                    )
                    .await
                    .map_err(|e| format!("Failed to dispatch OnStartup event: {:?}", e))?;
                }

                for other_guild in other_guilds {
                    regenerate_cache(context, data, other_guild).await?;

                    crate::dispatch::dispatch(
                        context, 
                        data, 
                        parse_event(&AntiraidEvent::OnStartup(vec![]))?,
                        other_guild
                    )
                    .await
                    .map_err(|e| format!("Failed to dispatch OnStartup event: {:?}", e))?;
                }
            }
        }
    }

    Ok(())
}

/// Gets all guilds with templates
pub fn get_all_guilds_with_templates() -> Vec<GuildId> {
    let mut guild_ids = Vec::new();

    for (guild_id, _) in TEMPLATES_CACHE.iter() {
        guild_ids.push(*guild_id);
    }

    guild_ids
}

pub async fn get_templates_with_event(
    guild_id: GuildId,
    event: &CreateEvent,
) -> Vec<Arc<Template>> {
    if let Some(templates) = TEMPLATES_CACHE.get(&guild_id).await {
        // `templates` should have $BUILTINS injected into it, so this is a simple for loop
        let mut matching_templates = Vec::with_capacity(templates.len());
        for template in templates.iter() {
            if template.should_dispatch(event) {
                matching_templates.push(template.clone());
            }
        }
        return matching_templates;
    } else {
        if USE_BUILTINS {
            if BUILTINS.should_dispatch(event) {
                let mut templates = Vec::with_capacity(1);
                templates.push(BUILTINS.clone());
                return templates;
            }
        }
        return Vec::with_capacity(0);
    }
}

pub async fn get_templates_by_name(
    guild_id: GuildId,
    name: &str,
) -> Vec<Arc<Template>> {
    if let Some(templates) = TEMPLATES_CACHE.get(&guild_id).await {
        // `templates` should have $BUILTINS injected into it, so this is a simple for loop
        let mut matching_templates = Vec::with_capacity(1);
        for template in templates.iter() {
            if template.name == name {
                matching_templates.push(template.clone());
            }
        }
        return matching_templates;
    } else {
        if USE_BUILTINS {
            if BUILTINS.name == name {
                let mut templates = Vec::with_capacity(1);
                templates.push(BUILTINS.clone());
                return templates;
            }
        }
        return Vec::with_capacity(0);
    }
}

pub async fn get_templates_with_event_scoped(
    guild_id: GuildId,
    event: &CreateEvent,
    scopes: &[String],
) -> Vec<Arc<Template>> {
    if let Some(templates) = TEMPLATES_CACHE.get(&guild_id).await {
        // `templates` should have $BUILTINS injected into it, so this is a simple for loop
        let mut matching_templates = Vec::with_capacity(templates.len());
        for template in templates.iter() {
            if template.should_dispatch_scoped(event, scopes) {
                matching_templates.push(template.clone());
            }
        }
        return matching_templates;
    } else {
        if USE_BUILTINS {
            if BUILTINS.should_dispatch_scoped(event, scopes) {
                let mut templates = Vec::with_capacity(1);
                templates.push(BUILTINS.clone());
                return templates;
            }
        }
        return Vec::with_capacity(0);
    }
}

/// Gets all templates for a guild
#[allow(dead_code)]
pub async fn get_all_guild_templates(guild_id: GuildId) -> Option<Arc<Vec<Arc<Template>>>> {
    match TEMPLATES_CACHE.get(&guild_id).await {
        Some(templates) => return Some(templates), // `templates` should have $BUILTINS injected into it
        None => {
            if USE_BUILTINS {
                log::debug!("Called get_all_guild_templates with USE_BUILTINS");
                let templates = BUILTINS_ARC_VEC.clone();
                return Some(templates); // Return the test base template
            }

            return None; // No templates found
        }
    }
}

/// Gets all expired keys across all guilds
pub fn get_all_expired_keys() -> Vec<(serenity::all::GuildId, Arc<KeyExpiry>)> {
    let mut expired = Vec::new();

    let now = chrono::Utc::now();
    for (guild_id, expiries) in KEY_EXPIRIES.iter() {
        for expiry in expiries.iter() {
            if expiry.expires_at <= now {
                expired.push((*guild_id, expiry.clone()));
            }
        }
    }

    expired
}

/// Sets up the initial template and key expiry cache
pub async fn setup(pool: &sqlx::PgPool) -> Result<(), crate::Error> {
    get_all_templates_from_db(pool).await?;
    get_all_key_expiries_from_db(pool).await?;
    Ok(())
}

/// Clears the template cache for a guild. This refetches the templates
/// into cache
pub async fn regenerate_cache(
    context: &serenity::all::Context,
    data: &Data,
    guild_id: GuildId,
) -> Result<(), crate::Error> {
    println!("Clearing cache for guild {}", guild_id);

    KEY_EXPIRIES.remove(&guild_id).await;

    // NOTE: if this call fails, bail out early and don't clear the cache to ensure old code at least runs
    let templates = get_all_guild_templates_from_db(guild_id, &data.pool).await?;
    get_all_guild_key_expiries_from_db(guild_id, &data.pool).await?;

    println!("Resyncing VMs");

    // Send a message to stop VMs running potentially outdated code
    let mut resync = false;
    if let Some(vm) = crate::templatingrt::POOL.get_guild_if_exists(guild_id)? {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        vm.send(ThreadRequest::Dispatch {
            callback: Some(tx),
            action: LuaVmAction::Stop {},
            guild_id,
        })?;
        let handle = RenderTemplateHandle { rx };
        let mvmr = handle.wait_timeout(MAX_TEMPLATES_RETURN_WAIT_TIME).await?;
        for result in mvmr.results {
            if result.is_error() {
                return Err(format!("Failed to clear cache in VM: {:?}", result.result).into());
            }
        }

        resync = true;
    } else {
        println!("No VMs to resync");
    }

    if resync {
        // Dispatch OnStartup events to all templates
        let templates = templates.iter().map(|t| t.name.clone()).collect();
        let create_event = match parse_event(&AntiraidEvent::OnStartup(templates)) {
            Ok(e) => e,
            Err(e) => {
                log::error!("Error parsing event: {:?}", e);
                return Ok(());
            }
        };

        crate::dispatch::dispatch(context, data, create_event, guild_id).await?;
    }

    Ok(())
}

async fn get_all_templates_from_db(pool: &sqlx::PgPool) -> Result<(), crate::Error> {
    #[derive(sqlx::FromRow)]
    struct GuildTemplatePartial {
        guild_id: String,
    }

    let partials: Vec<GuildTemplatePartial> =
        sqlx::query_as("SELECT guild_id FROM guild_templates GROUP BY guild_id")
            .fetch_all(pool)
            .await?;

    let mut templates: HashMap<serenity::all::GuildId, Vec<Arc<Template>>> =
        HashMap::with_capacity(partials.len());

    for partial in partials {
        let guild_id = partial.guild_id.parse()?;

        if let Ok(templates_vec) = Template::guild(guild_id, pool).await {
            let templates_vec = {
                let mut templates_found = Vec::with_capacity(templates_vec.len());
                let mut found_base = false;
                for template in templates_vec.into_iter() {
                    let template = template; // Make sure we mutably own
                    if template.name == BUILTINS_NAME {
                        found_base = true; // Mark that we have found the base template already
                    }

                    templates_found.push(Arc::new(template));
                }

                if !found_base && USE_BUILTINS {
                    templates_found.push(BUILTINS.clone()); // Add default test base template if not found
                }

                templates_found
            };

            templates.insert(guild_id, templates_vec);
        }
    }

    // Store the templates in the cache
    for (guild_id, templates) in templates {
        let templates = Arc::new(templates.clone());
        TEMPLATES_CACHE.insert(guild_id, templates.clone()).await;
    }

    Ok(())
}

async fn get_all_key_expiries_from_db(pool: &sqlx::PgPool) -> Result<(), crate::Error> {
    #[derive(sqlx::FromRow)]
    struct KeyExpiryPartial {
        guild_id: String,
        id: String,
        key: String,
        scopes: Vec<String>,
        expires_at: chrono::DateTime<chrono::Utc>,
    }

    let partials: Vec<KeyExpiryPartial> =
        sqlx::query_as("SELECT guild_id, id, key, scopes, expires_at FROM guild_templates_kv WHERE expires_at IS NOT NULL ORDER BY expires_at DESC")
            .fetch_all(pool)
            .await?;

    let mut expiries: HashMap<serenity::all::GuildId, Vec<Arc<KeyExpiry>>> = HashMap::new();

    for partial in partials {
        let guild_id = partial.guild_id.parse()?;

        let expiry = Arc::new(KeyExpiry {
            id: partial.id,
            key: partial.key,
            scopes: partial.scopes,
            expires_at: partial.expires_at,
        });

        if let Some(expiries_vec) = expiries.get_mut(&guild_id) {
            expiries_vec.push(expiry);
        } else {
            expiries.insert(guild_id, vec![expiry]);
        }
    }

    // Store the executions in the cache
    for (guild_id, expiry) in expiries {
        KEY_EXPIRIES.insert(guild_id, expiry.into()).await;
    }

    Ok(())
}

/// Gets all templates for a guild from the database
/// 
/// This will cache the templates in `TEMPLATES_CACHE` for future use.
/// 
/// Note that this method will *NOT* regenerate Lua VMs
pub async fn get_all_guild_templates_from_db(
    guild_id: GuildId,
    pool: &sqlx::PgPool,
) -> Result<Arc<Vec<Arc<Template>>>, crate::Error> {
    let mut templates_vec = Template::guild(guild_id, pool)
        .await?
        .into_iter()
        .map(|template| Arc::new(template))
        .collect::<Vec<_>>();

    if USE_BUILTINS {
        let mut found_base = false;
        for template in templates_vec.iter() {
            if template.name == BUILTINS_NAME {
                found_base = true;
                break;
            }
        }

        if !found_base {
            templates_vec.push(BUILTINS.clone());
        }
    }

    // Store the templates in the cache
    let templates = Arc::new(templates_vec);
    TEMPLATES_CACHE.insert(guild_id, templates.clone()).await;
    Ok(templates)
}

pub async fn get_all_guild_key_expiries_from_db(
    guild_id: GuildId,
    pool: &sqlx::PgPool,
) -> Result<(), crate::Error> {
    #[derive(sqlx::FromRow)]
    struct KeyExpiryPartial {
        id: String,
        key: String,
        scopes: Vec<String>,
        expires_at: chrono::DateTime<chrono::Utc>,
    }

    let executions_vec: Vec<KeyExpiryPartial> = sqlx::query_as(
        "SELECT id, key, scopes, expires_at FROM guild_templates_kv WHERE guild_id = $1 AND expires_at IS NOT NULL ORDER BY expires_at DESC",
    )
    .bind(guild_id.to_string())
    .fetch_all(pool)
    .await?;

    let executions_vec = executions_vec
        .into_iter()
        .map(|partial| {
            Arc::new(KeyExpiry {
                id: partial.id,
                key: partial.key,
                scopes: partial.scopes,
                expires_at: partial.expires_at,
            })
        })
        .collect::<Vec<_>>();

    // Store the executions in the cache
    KEY_EXPIRIES.insert(guild_id, executions_vec.into()).await;
    Ok(())
}

/// Removes keys with the given ID
pub async fn remove_key_expiry(
    guild_id: serenity::all::GuildId,
    id: &str,
    pool: &sqlx::PgPool,
) -> Result<(), crate::Error> {
    sqlx::query("DELETE FROM guild_templates_kv WHERE guild_id = $1 AND id = $2")
        .bind(guild_id.to_string())
        .bind(id)
        .execute(pool)
        .await?;

    // Reset gse cache for this guild
    get_all_guild_key_expiries_from_db(guild_id, pool).await?;

    Ok(())
}

/// Extend expiry of keys with the given ID
/// due to an error in their handling
pub async fn extend_key_expiry(
    guild_id: serenity::all::GuildId,
    id: &str,
    new_expiry: chrono::DateTime<chrono::Utc>,
    pool: &sqlx::PgPool,
) -> Result<(), crate::Error> {
    let mut tx = pool.begin().await?;

    // Check expiry_event_call_attempts
    let attempts: i64 = sqlx::query_scalar(
        "SELECT expiry_event_call_attempts FROM guild_templates_kv WHERE guild_id = $1 AND id = $2",
    )
    .bind(guild_id.to_string())
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;

    if attempts >= MAX_EXTENDS {
        return Err(format!(
            "Key expiry with ID {} has exceeded maximum extend attempts",
            id
        )
        .into());
    }

    sqlx::query("UPDATE guild_templates_kv SET expires_at = $1, expiry_event_call_attempts = expiry_event_call_attempts + 1 WHERE guild_id = $2 AND id = $3")
        .bind(new_expiry)
        .bind(guild_id.to_string())
        .bind(id)
        .execute(&mut *tx)
        .await?;

    tx.commit().await?;

    // Reset gse cache for this guild
    get_all_guild_key_expiries_from_db(guild_id, pool).await?;

    Ok(())
}
