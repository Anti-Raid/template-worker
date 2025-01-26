use super::template::Template;
use moka::future::Cache;
use serenity::all::GuildId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;

pub static TEMPLATES_CACHE: LazyLock<Cache<GuildId, Arc<Vec<Arc<Template>>>>> =
    LazyLock::new(|| Cache::builder().build());

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
    let partials = sqlx::query!("SELECT name, guild_id FROM guild_templates",)
        .fetch_all(pool)
        .await?;

    let mut templates: HashMap<serenity::all::GuildId, Vec<Arc<Template>>> = HashMap::new();

    for partial in partials {
        let guild_id = partial.guild_id.parse()?;
        let template = Template::guild(guild_id, &partial.name, pool).await?;

        if let Some(templates_vec) = templates.get_mut(&guild_id) {
            templates_vec.push(Arc::new(template));
        } else {
            templates.insert(guild_id, vec![Arc::new(template)]);
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
    let partials = sqlx::query!(
        "SELECT name FROM guild_templates WHERE guild_id = $1",
        guild_id.to_string()
    )
    .fetch_all(pool)
    .await?;

    let mut templates = Vec::new();

    for partial in partials {
        let template = Template::guild(guild_id, &partial.name, pool).await?;
        templates.push(Arc::new(template));
    }

    // Store the templates in the cache
    let templates = Arc::new(templates.clone());
    TEMPLATES_CACHE.insert(guild_id, templates.clone()).await;
    Ok(())
}
