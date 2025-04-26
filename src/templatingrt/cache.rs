use super::template::Template;
use super::vm_manager::{get_lua_vm_if_exists, ArLuaHandle};
use super::{LuaVmAction, RenderTemplateHandle, MAX_TEMPLATES_RETURN_WAIT_TIME};
use moka::future::Cache;
use serenity::all::GuildId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use khronos_runtime::primitives::event::CreateEvent;

// Test base will be used for builtins in the future

// Exec simple with wait
fn str_to_map(s: &str) -> std::collections::HashMap<String, Arc<String>> {
    let mut map = std::collections::HashMap::new();
    map.insert("init.luau".to_string(), Arc::new(s.to_string()));
    map
}

const TEST_BASE: LazyLock<Arc<Template>> = LazyLock::new(|| Arc::new(Template {
    content: str_to_map("error('test')"),
    name: "test".to_string(),
    events: vec!["MESSAGE".to_string(), "INTERACTION_CREATE".to_string()],
    ..Default::default()
}));
const USE_TEST_BASE: bool = true;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct ScheduledExecution {
    pub template_name: String,
    pub id: String,
    pub data: serde_json::Value,
    pub run_at: chrono::DateTime<chrono::Utc>,
}

/// This should be in descending order of run_at
pub static SCHEDULED_EXECUTIONS: LazyLock<Cache<GuildId, Arc<Vec<Arc<ScheduledExecution>>>>> =
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
        return true;
    }
    TEMPLATES_CACHE.contains_key(&guild_id)
}

pub async fn has_templates_with_event(
    guild_id: GuildId,
    event: &CreateEvent,
) -> bool {
    if let Some(templates) = TEMPLATES_CACHE.get(&guild_id).await {
        for template in templates.iter() {
            if template.should_dispatch(event) {
                return true;
            }
        }
    }

    if USE_TEST_BASE {
        return TEST_BASE.should_dispatch(event);
    }

    false
}

/// Gets all templates for a guild
#[allow(dead_code)]
pub async fn get_all_guild_templates(guild_id: GuildId) -> Option<Arc<Vec<Arc<Template>>>> {
    if USE_TEST_BASE {
        match TEMPLATES_CACHE.get(&guild_id).await {
            Some(templates) => return Some(templates),
            None => {
                let templates = Arc::new(vec![TEST_BASE.clone()]);
                return Some(templates);
            }
        }
    }
    
    TEMPLATES_CACHE.get(&guild_id).await
}

/// Gets all expired scheduled executions across all guilds
pub fn get_all_expired_scheduled_executions() -> Vec<(serenity::all::GuildId, Arc<ScheduledExecution>)> {
    let mut expired = Vec::new();

    let now = chrono::Utc::now();
    for (guild_id, executions) in SCHEDULED_EXECUTIONS.iter() {
        for execution in executions.iter() {
            if execution.run_at <= now {
                expired.push((*guild_id, execution.clone()));
            }
        }
    }

    expired
}

/// Gets a guild template by name
pub async fn get_guild_template(guild_id: GuildId, name: &str) -> Option<Arc<Template>> {
    if USE_TEST_BASE && name == "test" {
        return Some(TEST_BASE.clone());
    }

    let templates = TEMPLATES_CACHE.get(&guild_id).await?;

    for t in templates.iter() {
        if t.name == name {
            return Some(t.clone());
        }
    }

    None
}

/// Sets up the initial template and scheduled execution cache
pub async fn setup(pool: &sqlx::PgPool) -> Result<(), silverpelt::Error> {
    get_all_templates_from_db(pool).await?;
    get_all_scheduled_executions_from_db(pool).await?;
    Ok(())
}

/// Clears the template cache for a guild. This refetches the templates
/// into cache
pub async fn regenerate_cache(
    guild_id: GuildId,
    pool: &sqlx::PgPool,
) -> Result<(), silverpelt::Error> {
    println!("Clearing cache for guild {}", guild_id);

    TEMPLATES_CACHE.remove(&guild_id).await;
    SCHEDULED_EXECUTIONS.remove(&guild_id).await;

    // NOTE: if this call fails, bail out early and don't clear the cache to ensure old code at least runs
    get_all_guild_templates_from_db(guild_id, pool).await?;
    get_all_guild_scheduled_executions_from_db(guild_id, pool).await?;

    println!("Resyncing VMs");

    // Send a message to clear the cache in the VMs
    if let Some(vm) = get_lua_vm_if_exists(guild_id) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        vm.send_action(LuaVmAction::ClearCache {}, tx)?;
        let handle = RenderTemplateHandle { rx };
        let Some(mvmr) = handle.wait_timeout(MAX_TEMPLATES_RETURN_WAIT_TIME).await? else {
            return Err("Timed out waiting for templates to clear from VMs".into());
        };

        for result in mvmr.results {
            if result.is_error() {
                return Err(format!("Failed to clear cache in VM: {:?}", result.result).into());
            }
        }
    } else {
        println!("No VMs to resync");
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

    let mut templates: HashMap<serenity::all::GuildId, Vec<Arc<Template>>> = HashMap::with_capacity(partials.len());

    for partial in partials {
        let guild_id = partial.guild_id.parse()?;

        if let Ok(templates_vec) = Template::guild(guild_id, pool).await {
            let mut templates_vec = templates_vec.into_iter().map(Arc::new).collect::<Vec<_>>();
            if USE_TEST_BASE {
                templates_vec.push(TEST_BASE.clone());
            }
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

async fn get_all_scheduled_executions_from_db(pool: &sqlx::PgPool) -> Result<(), silverpelt::Error> {
    #[derive(sqlx::FromRow)]
    struct ScheduledExecutionPartial {
        guild_id: String,
        template_name: String,
        exec_id: String,
        data: serde_json::Value,
        run_at: chrono::DateTime<chrono::Utc>,
    }

    let partials: Vec<ScheduledExecutionPartial> =
        sqlx::query_as("SELECT guild_id, exec_id, data, run_at, template_name FROM scheduled_executions ORDER BY run_at DESC")
            .fetch_all(pool)
            .await?;

    let mut executions: HashMap<serenity::all::GuildId, Vec<Arc<ScheduledExecution>>> =
        HashMap::new();

    for partial in partials {
        let guild_id = partial.guild_id.parse()?;

        let execution = Arc::new(ScheduledExecution {
            id: partial.exec_id,
            template_name: partial.template_name,
            data: partial.data,
            run_at: partial.run_at,
        });

        if let Some(executions_vec) = executions.get_mut(&guild_id) {
            executions_vec.push(execution);
        } else {
            executions.insert(guild_id, vec![execution]);
        }
    }

    // Store the executions in the cache
    for (guild_id, executions) in executions {
        SCHEDULED_EXECUTIONS.insert(guild_id, executions.into()).await;
    }

    Ok(())
}

async fn get_all_guild_templates_from_db(
    guild_id: GuildId,
    pool: &sqlx::PgPool,
) -> Result<(), silverpelt::Error> {
    let templates_vec = Template::guild(guild_id, pool)
        .await?
        .into_iter()
        .map(Arc::new)
        .collect::<Vec<_>>();

    // Store the templates in the cache
    let templates = Arc::new(templates_vec);
    TEMPLATES_CACHE.insert(guild_id, templates.clone()).await;
    Ok(())
}

pub async fn get_all_guild_scheduled_executions_from_db(
    guild_id: GuildId,
    pool: &sqlx::PgPool,
) -> Result<(), silverpelt::Error> {
    #[derive(sqlx::FromRow)]
    struct ScheduledExecutionPartial {
        exec_id: String,
        template_name: String,
        data: serde_json::Value,
        run_at: chrono::DateTime<chrono::Utc>,
    }

    let executions_vec: Vec<ScheduledExecutionPartial> = sqlx::query_as(
        "SELECT exec_id, template_name, data, run_at FROM scheduled_executions WHERE guild_id = $1 ORDER BY run_at DESC",
    )
    .bind(guild_id.to_string())
    .fetch_all(pool)
    .await?;

    let executions_vec = executions_vec
        .into_iter()
        .map(|partial| Arc::new(ScheduledExecution {
            id: partial.exec_id,
            template_name: partial.template_name,
            data: partial.data,
            run_at: partial.run_at,
        }))
        .collect::<Vec<_>>();

    // Store the executions in the cache
    SCHEDULED_EXECUTIONS.insert(guild_id, executions_vec.into()).await;
    Ok(())
}

/// Removes all scheduled execution with the given ID
pub async fn remove_scheduled_execution(
    guild_id: serenity::all::GuildId,
    id: &str,
    pool: &sqlx::PgPool,
) -> Result<(), silverpelt::Error> {
    sqlx::query(
        "DELETE FROM scheduled_executions WHERE guild_id = $1 AND exec_id = $2",
    )
    .bind(guild_id.to_string())
    .bind(id)
    .execute(pool)
    .await?;

    // Reset gse cache for this guild
    get_all_guild_scheduled_executions_from_db(guild_id, pool).await?;

     Ok(())
}
