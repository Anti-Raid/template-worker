use crate::core::page::Page;
use crate::Template;
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

pub static PAGES: LazyLock<scc::HashMap<GuildId, Vec<Page>>> = LazyLock::new(scc::HashMap::new);

/// Gets all templates for a guild
#[allow(dead_code)]
pub async fn get_all_guild_templates(
    guild_id: GuildId,
    pool: &sqlx::PgPool,
) -> Result<Arc<Vec<Arc<Template>>>, crate::Error> {
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
        let template = crate::Template::guild(guild_id, &name, pool).await?;
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
) -> Result<Arc<Template>, crate::Error> {
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
    PAGES.remove_async(&guild_id).await;
}

/// Adds a page to the page cache
pub async fn add_page(guild_id: GuildId, page: Page) -> Result<(), crate::Error> {
    match PAGES.get_async(&guild_id).await {
        Some(mut pages) => {
            for existing_page in pages.iter() {
                if existing_page.page_id == page.page_id {
                    return Err("Page already exists".into());
                }
            }
            pages.push(page);
        }
        None => {
            let pages = vec![page];
            PAGES.upsert_async(guild_id, pages).await;
        }
    }

    Ok(())
}

/// Adds a page to the page cache
pub fn add_page_sync(guild_id: GuildId, page: Page) -> Result<(), crate::Error> {
    match PAGES.get(&guild_id) {
        Some(mut pages) => {
            for existing_page in pages.iter() {
                if existing_page.page_id == page.page_id {
                    return Err("Page already exists".into());
                }
            }
            pages.push(page);
        }
        None => {
            let pages = vec![page];
            PAGES.upsert(guild_id, pages);
        }
    }

    Ok(())
}

/// Returns a setting from the page cache given the setting ID
pub async fn get_setting(
    guild_id: GuildId,
    setting_id: &str,
) -> Option<ar_settings::types::Setting> {
    PAGES
        .read_async(&guild_id, |_, v| {
            for page in v.iter() {
                for setting in page.settings.iter() {
                    if setting.id == setting_id {
                        return Some(setting.clone());
                    }
                }
            }

            None
        })
        .await?
}

/// Takes out the page from the page cache by page ID
pub async fn take_page(guild_id: GuildId, page_id: String) -> Result<Page, crate::Error> {
    match PAGES.get_async(&guild_id).await {
        Some(mut pages) => {
            let mut index = None;
            for (i, page) in pages.iter().enumerate() {
                if page.page_id == page_id {
                    index = Some(i);
                    break;
                }
            }

            if let Some(index) = index {
                let page = (*pages).remove(index);
                Ok(page)
            } else {
                Err("Page not found".into())
            }
        }
        None => Err("No pages found".into()),
    }
}

/// Takes out the page from the page cache by page ID
pub fn take_page_sync(guild_id: GuildId, page_id: String) -> Result<Page, crate::Error> {
    match PAGES.get(&guild_id) {
        Some(mut pages) => {
            let mut index = None;
            for (i, page) in pages.iter().enumerate() {
                if page.page_id == page_id {
                    index = Some(i);
                    break;
                }
            }

            if let Some(index) = index {
                let page = (*pages).remove(index);
                Ok(page)
            } else {
                Err("Page not found".into())
            }
        }
        None => Err("No pages found".into()),
    }
}

/// Removes a page from the page cache by page ID
pub async fn remove_page(guild_id: GuildId, page_id: String) -> Result<(), crate::Error> {
    match PAGES.get_async(&guild_id).await {
        Some(mut pages) => {
            let mut index = None;
            for (i, page) in pages.iter().enumerate() {
                if page.page_id == page_id {
                    index = Some(i);
                    break;
                }
            }

            if let Some(index) = index {
                (*pages).remove(index);
            } else {
                return Err("Page not found".into());
            }
        }
        None => return Err("No pages found".into()),
    }

    Ok(())
}

/// Removes a page from the page cache by page ID
pub fn remove_page_sync(guild_id: GuildId, page_id: String) -> Result<(), crate::Error> {
    match PAGES.get(&guild_id) {
        Some(mut pages) => {
            let mut index = None;
            for (i, page) in pages.iter().enumerate() {
                if page.page_id == page_id {
                    index = Some(i);
                    break;
                }
            }

            if let Some(index) = index {
                (*pages).remove(index);
            } else {
                return Err("Page not found".into());
            }
        }
        None => return Err("No pages found".into()),
    }

    Ok(())
}
