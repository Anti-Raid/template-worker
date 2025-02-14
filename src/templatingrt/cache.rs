use super::template::Template;
use moka::future::Cache;
use serenity::all::GuildId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;

pub static TEMPLATES_CACHE: LazyLock<Cache<GuildId, Arc<Vec<Arc<Template>>>>> =
    LazyLock::new(|| Cache::builder().build());

/// Gets all guilds with templates
pub fn get_all_guilds() -> Vec<GuildId> {
    let mut templates = Vec::new();

    for (guild_id, _) in TEMPLATES_CACHE.iter() {
        templates.push(*guild_id);
    }

    templates
}

/// Gets all templates for a guild
#[allow(dead_code)]
pub async fn get_all_guild_templates(guild_id: GuildId) -> Option<Arc<Vec<Arc<Template>>>> {
    TEMPLATES_CACHE.get(&guild_id).await
}

/// Gets a guild template by name
pub async fn get_guild_template(guild_id: GuildId, name: &str) -> Option<Arc<Template>> {
    let templates = TEMPLATES_CACHE.get(&guild_id).await?;

    for t in templates.iter() {
        if t.name == name {
            return Some(t.clone());
        }
    }

    None
}

/// Sets up the initial template cache
pub async fn setup(pool: &sqlx::PgPool) -> Result<(), silverpelt::Error> {
    get_all_templates_from_db(pool).await?;
    Ok(())
}

/// Clears the template cache for a guild. This refetches the templates
/// into cache
pub async fn regenerate_cache(guild_id: GuildId, pool: &sqlx::PgPool) {
    TEMPLATES_CACHE.remove(&guild_id).await;

    let _ = get_all_guild_templates_from_db(guild_id, pool).await;
}

async fn get_all_templates_from_db(pool: &sqlx::PgPool) -> Result<(), silverpelt::Error> {
    let partials = sqlx::query!("SELECT guild_id FROM guild_templates GROUP BY guild_id")
        .fetch_all(pool)
        .await?;

    let mut templates: HashMap<serenity::all::GuildId, Vec<Arc<Template>>> = HashMap::new();

    for partial in partials {
        let guild_id = partial.guild_id.parse()?;

        if let Ok(templates_vec) = Template::guild(guild_id, pool).await {
            let templates_vec = templates_vec.into_iter().map(Arc::new).collect::<Vec<_>>();
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
