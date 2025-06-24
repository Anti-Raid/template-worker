use super::template::Template;
use super::{LuaVmAction, RenderTemplateHandle, ThreadRequest, MAX_TEMPLATES_RETURN_WAIT_TIME};
use crate::dispatch::parse_event;
use antiraid_types::ar_event::AntiraidEvent;
use khronos_runtime::primitives::event::CreateEvent;
use moka::future::Cache;
use serenity::all::GuildId;
use silverpelt::data::Data;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use vfs::FileSystem;

// Test base will be used for builtins in the future

// Exec simple with wait
fn str_to_fs(s: &str) -> vfs::MemoryFS {
    let fs = vfs::MemoryFS::new();
    fs.create_file("/init.luau")
        .unwrap()
        .write_all(s.as_bytes())
        .unwrap();
    fs
}

// Replace this with the new builtins template once ready to deploy
pub const TEST_BASE_NAME: &str = "$test_base";
pub static TEST_BASE: LazyLock<Arc<Template>> = LazyLock::new(|| {
    let mut templ = Template {
        content: str_to_fs("local evt, ctx = ...\nif evt.name == 'INTERACTION_CREATE' then error(ctx.guild_id) end"),
        name: TEST_BASE_NAME.to_string(),
        events: vec!["INTERACTION_CREATE".to_string()],

        ..Default::default()
    };

    templ.prepare_ready_fs();

    Arc::new(templ)
});
pub static TEST_BASE_ARC_VEC: LazyLock<Arc<Vec<Arc<Template>>>> =
    LazyLock::new(|| Arc::new(vec![TEST_BASE.clone()]));
pub const USE_TEST_BASE: bool = true;

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

/// Gets all guilds with templates
pub fn get_all_guilds_with_templates() -> Vec<GuildId> {
    let mut guild_ids = Vec::new();

    for (guild_id, _) in TEMPLATES_CACHE.iter() {
        guild_ids.push(*guild_id);
    }

    guild_ids
}

/// Returns if a guild has any templates
pub fn has_templates(guild_id: GuildId) -> bool {
    if USE_TEST_BASE {
        return true; // The quick answer here is: yes
    }
    TEMPLATES_CACHE.contains_key(&guild_id)
}

pub async fn get_templates_with_event(
    guild_id: GuildId,
    event: &CreateEvent,
) -> Vec<Arc<Template>> {
    if let Some(templates) = TEMPLATES_CACHE.get(&guild_id).await {
        // `templates` should have $test_base injected into it, so this is a simple for loop
        let mut matching_templates = Vec::with_capacity(templates.len());
        for template in templates.iter() {
            if template.should_dispatch(event) {
                matching_templates.push(template.clone());
            }
        }
        return matching_templates;
    } else {
        if USE_TEST_BASE {
            if TEST_BASE.should_dispatch(event) {
                let mut templates = Vec::with_capacity(1);
                templates.push(TEST_BASE.clone());
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
        // `templates` should have $test_base injected into it, so this is a simple for loop
        let mut matching_templates = Vec::with_capacity(templates.len());
        for template in templates.iter() {
            if template.should_dispatch_scoped(event, scopes) {
                matching_templates.push(template.clone());
            }
        }
        return matching_templates;
    } else {
        if USE_TEST_BASE {
            if TEST_BASE.should_dispatch_scoped(event, scopes) {
                let mut templates = Vec::with_capacity(1);
                templates.push(TEST_BASE.clone());
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
        Some(templates) => return Some(templates), // `templates` should have $test_base injected into it
        None => {
            if USE_TEST_BASE {
                log::debug!("Called get_all_guild_templates with USE_TEST_BASE");
                let templates = TEST_BASE_ARC_VEC.clone();
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

/// Gets a guild template by name
pub async fn get_guild_template(guild_id: GuildId, name: &str) -> Option<Arc<Template>> {
    match TEMPLATES_CACHE.get(&guild_id).await {
        Some(templates) => {
            // The `templates` variable should anyways have $test_base injected into it
            for t in templates.iter() {
                if t.name == name {
                    return Some(t.clone());
                }
            }

            return None;
        }
        None => {
            // The server always has the test base template so ensure we return it
            if USE_TEST_BASE && name == TEST_BASE_NAME {
                return Some(TEST_BASE.clone());
            }

            return None;
        }
    }
}

/// Sets up the initial template and key expiry cache
pub async fn setup(pool: &sqlx::PgPool) -> Result<(), silverpelt::Error> {
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
) -> Result<(), silverpelt::Error> {
    println!("Clearing cache for guild {}", guild_id);

    KEY_EXPIRIES.remove(&guild_id).await;

    // NOTE: if this call fails, bail out early and don't clear the cache to ensure old code at least runs
    let templates = get_all_guild_templates_from_db(
        guild_id,
        &data.pool,
        TEMPLATES_CACHE.remove(&guild_id).await,
    )
    .await?;
    get_all_guild_key_expiries_from_db(guild_id, &data.pool).await?;

    println!("Resyncing VMs");

    // Send a message to stop VMs running potentially outdated code
    let mut resync = false;
    if let Some(vm) = crate::templatingrt::POOL.get_guild_if_exists(guild_id)? {
        let (tx, rx) = tokio::sync::oneshot::channel();
        vm.send(ThreadRequest::Dispatch {
            callback: tx,
            action: LuaVmAction::Stop {},
            guild_id,
        })?;
        let handle = RenderTemplateHandle { rx };
        let Some(mvmr) = handle.wait_timeout(MAX_TEMPLATES_RETURN_WAIT_TIME).await? else {
            return Err("Timed out waiting for templates to clear from VMs".into());
        };

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

async fn get_all_templates_from_db(pool: &sqlx::PgPool) -> Result<(), silverpelt::Error> {
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

        let old_templates = TEMPLATES_CACHE.get(&guild_id).await;

        if let Ok(templates_vec) = Template::guild(guild_id, pool).await {
            let templates_vec = {
                let mut templates_found = Vec::with_capacity(templates_vec.len());
                let mut found_base = false;
                for template in templates_vec.into_iter() {
                    let mut template = template; // Make sure we mutably own
                    if template.name == TEST_BASE_NAME {
                        found_base = true; // Mark that we have found the base template already
                    }

                    // Get the content of old template
                    // TODO: Optimize this logic maybe?
                    if let Some(ref old_templates) = old_templates {
                        for old_template in old_templates.iter() {
                            if template.name == old_template.name {
                                // Copy over filesystem ref and make them point to the same thing
                                old_template
                                    .content
                                    .take_from_filesystem(&template.content)?; // Propogate error upwards as this should never happen outside of poisoned RwLock
                                template.content = old_template.content.clone();
                                break;
                            }
                        }
                    }

                    template.prepare_ready_fs();

                    templates_found.push(Arc::new(template));
                }

                if !found_base && USE_TEST_BASE {
                    templates_found.push(TEST_BASE.clone()); // Add default test base template if not found
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

async fn get_all_key_expiries_from_db(pool: &sqlx::PgPool) -> Result<(), silverpelt::Error> {
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

async fn get_all_guild_templates_from_db(
    guild_id: GuildId,
    pool: &sqlx::PgPool,
    old: Option<Arc<Vec<Arc<Template>>>>,
) -> Result<Arc<Vec<Arc<Template>>>, silverpelt::Error> {
    let mut templates_vec = Template::guild(guild_id, pool)
        .await?
        .into_iter()
        .collect::<Vec<_>>();

    // If we have old templates, we need to copy over the filesystem
    if let Some(old_templates) = old {
        for template in templates_vec.iter_mut() {
            for old_template in old_templates.iter() {
                if template.name == old_template.name {
                    // Copy over filesystem ref and make them point to the same thing
                    old_template
                        .content
                        .take_from_filesystem(&template.content)?;
                    template.content = old_template.content.clone();
                    break;
                }
            }
        }
    }

    // Prepare the ready filesystem
    let mut templates_vec = templates_vec
        .into_iter()
        .map(|template| {
            let mut template = template;
            template.prepare_ready_fs();
            Arc::new(template)
        })
        .collect::<Vec<_>>();

    if USE_TEST_BASE {
        let mut found_base = false;
        for template in templates_vec.iter() {
            if template.name == TEST_BASE_NAME {
                found_base = true;
                break;
            }
        }

        if !found_base {
            templates_vec.push(TEST_BASE.clone());
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
) -> Result<(), silverpelt::Error> {
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
) -> Result<(), silverpelt::Error> {
    sqlx::query("DELETE FROM guild_templates_kv WHERE guild_id = $1 AND id = $2")
        .bind(guild_id.to_string())
        .bind(id)
        .execute(pool)
        .await?;

    // Reset gse cache for this guild
    get_all_guild_key_expiries_from_db(guild_id, pool).await?;

    Ok(())
}
