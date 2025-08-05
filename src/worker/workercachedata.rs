use std::{collections::HashMap, hash::Hash, sync::Arc};

use crate::templatingrt::template::Template;

use super::builtins::{BUILTINS_NAME, USE_BUILTINS, BUILTINS};
use super::workervmmanager::Id;

use khronos_runtime::primitives::event::CreateEvent;
use moka::future::Cache;
use serenity::all::GuildId;

#[derive(Clone)]
struct CacheEntry<K, V> 
where K: Send + Sync + Eq + Hash + 'static,
      V: Send + Sync + Clone + 'static {
    data: Arc<Cache<K, V>>
}

impl<K, V> CacheEntry<K, V> 
where K: Send + Sync + Eq + Hash + 'static,
      V: Send + Sync + Clone + 'static {
    fn new() -> Self {
        Self {
            // Unbounded permanent cache
            data: Arc::new(Cache::builder().build())
        }
    }

    async fn get(&self, key: &K) -> Option<V> {
        self.data.get(key).await
    }

    async fn insert(&self, key: K, value: V) {
        self.data.insert(key, value).await;
    }

    async fn remove(&self, key: &K) {
        self.data.invalidate(key).await;
    }

    fn iter(&self) -> impl Iterator<Item = (Arc<K>, V)> + use<'_, K, V> {
        self.data.iter()
    }
}

type ArcVec<T> = Arc<Vec<Arc<T>>>;

#[derive(Debug)]
pub struct KeyExpiry {
    pub id: String,
    pub key: String,
    pub scopes: Vec<String>,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

/// WorkerCacheData stores cache related data for templates and associated data (like key expiries)
/// 
/// NOTE: WorkerCache (WIP) will use WorkerCacheData on top of WorkerVmManager to allow for cache regeneration etc
/// 
/// WorkerCacheData is explicitly thread safe (and is one of the few parts of workers that is thread safe)
#[derive(Clone)]
pub struct WorkerCacheData {
    templates: CacheEntry<Id, ArcVec<Template>>, // Maps template names to their associated keys
    key_expiries: CacheEntry<Id, ArcVec<KeyExpiry>>, // Maps guild id to key expiries
}

impl WorkerCacheData {
    /// Creates a new WorkerCacheData instance
    ///
    /// This will also set up the initial cache from the database
    pub async fn new(pool: &sqlx::PgPool) -> Result<Self, crate::Error> {
        let data = Self {
            templates: CacheEntry::new(),
            key_expiries: CacheEntry::new(),
        };

        // Setup initial cache from database
        data.setup(pool).await?;

        Ok(data)
    }

    /// Sets up the initial template and key expiry cache
    async fn setup(&self, pool: &sqlx::PgPool) -> Result<(), crate::Error> {
        self.populate_templates_from_db(pool).await?;
        self.populate_key_expiries_from_db(pool).await?;
        Ok(())
    }

    /// Regenerates the templates cache from the database for a tenant
    pub async fn regenerate_templates_for(&self, pool: &sqlx::PgPool, id: Id) -> Result<(), crate::Error> {
        match id {
            Id::GuildId(guild_id) => {
                self.repopulate_guild_templates_from_db(guild_id, pool).await?;
            }
            _ => {
                return Err("Cannot regenerate templates for non-guild IDs".into());
            }
        }

        Ok(())
    }

    /// Regenerates key expiries for a tenant
    pub async fn regenerate_key_expiries_for(&self, pool: &sqlx::PgPool, id: Id) -> Result<(), crate::Error> {
        match id {
            Id::GuildId(guild_id) => {
                self.repopulate_guild_key_expiries_from_db(guild_id, pool).await?;
            }
            _ => {
                return Err("Cannot regenerate key expiries for non-guild IDs".into());
            }
        }

        Ok(())
    }

    /// Gets all templates matching the event given by `CreateEvent`
    pub async fn get_templates_with_event(
        &self,
        id: Id,
        event: &CreateEvent,
    ) -> Vec<Arc<Template>> {
        self.get_templates_by_predicate(id, |template| {
            template.should_dispatch(event)
        }).await
    }

    /// Gets all templates matching the name given
    pub async fn get_templates_by_name(
        &self,
        id: Id,
        name: &str,
    ) -> Vec<Arc<Template>> {
        self.get_templates_by_predicate(id, |template| {
            template.name == name
        }).await
    }

    /// Gets all templates matching the event given by `CreateEvent` and the scopes
    pub async fn get_templates_with_event_scoped(
        &self,
        id: Id,
        event: &CreateEvent,
        scopes: &[String],
    ) -> Vec<Arc<Template>> {
        self.get_templates_by_predicate(id, |template| {
            template.should_dispatch_scoped(event, scopes)
        }).await
    }

    /// Helper method to get templates by a predicate
    pub async fn get_templates_by_predicate(&self, id: Id, predicate: impl Fn(&Arc<Template>) -> bool) -> Vec<Arc<Template>> {
        if let Some(templates) = self.templates.get(&id).await {
            templates.iter().filter(|t| predicate(t)).cloned().collect()
        } else {
            if USE_BUILTINS {
                if predicate(&BUILTINS) {
                    let mut templates = Vec::with_capacity(1);
                    templates.push(BUILTINS.clone());
                    return templates;
                }
            }
            Vec::with_capacity(0)
        }
    }

    /// Returns all currently expired keys for a tenant
    pub fn get_all_expired_keys(&self) -> Vec<(Id, Arc<KeyExpiry>)> {
        let mut expired = Vec::new();

        let now = chrono::Utc::now();
        for (id, expiries) in self.key_expiries.iter() {
            for expiry in expiries.iter() {
                if expiry.expires_at <= now {
                    expired.push((*id, expiry.clone()));
                }
            }
        }

        expired
    }

    /// Populates the templates cache from the database
    /// 
    /// Currently only handles guild templates
    async fn populate_templates_from_db(&self, pool: &sqlx::PgPool) -> Result<(), crate::Error> {
        #[derive(sqlx::FromRow)]
        struct GuildTemplatePartial {
            guild_id: String,
        }

        let partials: Vec<GuildTemplatePartial> =
            sqlx::query_as("SELECT guild_id FROM guild_templates GROUP BY guild_id")
            .fetch_all(pool)
            .await?;

        for partial in partials {
            let guild_id = partial.guild_id.parse()?;

            if let Ok(templates_vec) = Template::guild(guild_id, pool).await {
                let templates_vec = {
                    let mut templates_found = Vec::with_capacity(templates_vec.len());
                    let mut found_base = false;
                    for template in templates_vec.into_iter() {
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

                self.templates.insert(Id::GuildId(guild_id), Arc::new(templates_vec)).await;
            }
        }

        Ok(())
    }

    /// Gets all key expiries from the database and stores them in the cache
    async fn populate_key_expiries_from_db(&self, pool: &sqlx::PgPool) -> Result<(), crate::Error> {
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

        let mut expiries: HashMap<Id, Vec<Arc<KeyExpiry>>> = HashMap::new();

        for partial in partials {
            let guild_id = partial.guild_id.parse()?;

            let expiry = Arc::new(KeyExpiry {
                id: partial.id,
                key: partial.key,
                scopes: partial.scopes,
                expires_at: partial.expires_at,
            });

            let id = Id::GuildId(guild_id);
            if let Some(expiries_vec) = expiries.get_mut(&id) {
                expiries_vec.push(expiry);
            } else {
                expiries.insert(id, vec![expiry]);
            }
        }

        // Store the executions in the cache
        for (id, expiry) in expiries {
            self.key_expiries.insert(id, expiry.into()).await;
        }

        Ok(())
    }

    /// Gets all templates for a guild from the database and stores them in the cache
    /// replacing the existing templates in cache
    /// 
    /// Note that this method will *NOT* regenerate Lua VMs
    async fn repopulate_guild_templates_from_db(
        &self,
        guild_id: GuildId,
        pool: &sqlx::PgPool,
    ) -> Result<(), crate::Error> {
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
        self.templates.insert(Id::GuildId(guild_id), templates).await;
        Ok(())
    }

    /// Repopulates the key expiries for a guild from the database
    /// 
    /// This will replace the existing key expiries in cache
    async fn repopulate_guild_key_expiries_from_db(
        &self,
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
        self.key_expiries.insert(Id::GuildId(guild_id), executions_vec.into()).await;
        Ok(())
    }
}

// Assert that WorkerCacheData is Send + Sync + Clone
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync + Clone>() {}
    assert_send_sync_clone::<WorkerCacheData>();
};
