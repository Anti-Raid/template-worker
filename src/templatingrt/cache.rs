use super::template::Template;
use moka::future::Cache;
use serenity::all::GuildId;
use std::sync::Arc;
use std::sync::LazyLock;
use std::time::Duration;

pub static TEMPLATES_CACHE: LazyLock<Cache<GuildId, Arc<Vec<Arc<Template>>>>> =
    LazyLock::new(|| {
        Cache::builder()
            .support_invalidation_closures()
            .time_to_idle(Duration::from_secs(60 * 5)) // Expire the audit log sink cache after 5 minutes
            .build()
    });

/// Gets all templates for a guild
#[allow(dead_code)]
pub async fn get_all_guild_templates(
    guild_id: GuildId,
    pool: &sqlx::PgPool,
) -> Result<Arc<Vec<Arc<Template>>>, silverpelt::Error> {
    if let Some(templates) = TEMPLATES_CACHE.get(&guild_id).await {
        return Ok(templates.clone());
    }

    let names = sqlx::query!(
        "SELECT name FROM guild_templates WHERE guild_id = $1",
        guild_id.to_string()
    )
    .fetch_all(pool)
    .await?
    .into_iter()
    .map(|r| r.name)
    .collect::<Vec<String>>();

    let mut templates = Vec::new();

    for name in names {
        let template = Template::guild(guild_id, &name, pool).await?;
        templates.push(Arc::new(template));
    }

    // Store the templates in the cache
    let templates = Arc::new(templates.clone());
    TEMPLATES_CACHE.insert(guild_id, templates.clone()).await;

    Ok(templates)
}

/// Gets a guild template by name
pub async fn get_guild_template(
    guild_id: GuildId,
    name: &str,
    pool: &sqlx::PgPool,
) -> Result<Arc<Template>, silverpelt::Error> {
    let template = get_all_guild_templates(guild_id, pool).await?;

    for t in template.iter() {
        if t.name == name {
            return Ok(t.clone());
        }
    }

    Err("Template not found".into())
}

/// Clears the template cache for a guild
pub async fn clear_cache(guild_id: GuildId) {
    TEMPLATES_CACHE.remove(&guild_id).await;
}
