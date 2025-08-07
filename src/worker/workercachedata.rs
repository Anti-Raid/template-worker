use std::{hash::Hash, sync::Arc};

use super::template::Template;

use super::builtins::{USE_BUILTINS, BUILTINS};
use super::workervmmanager::Id;
use super::workerdb::WorkerDB;

use khronos_runtime::primitives::event::CreateEvent;
use moka::future::Cache;

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

    async fn remove(&self, key: &K) -> Option<V> {
        self.data.remove(key).await
    }

    fn iter(&self) -> impl Iterator<Item = (Arc<K>, V)> + use<'_, K, V> {
        self.data.iter()
    }
}

type ArcVec<T> = Arc<Vec<Arc<T>>>;

#[derive(Debug, Clone)]
#[allow(unused)]
pub enum DeferredCacheRegenerationMode {
    /// Only need to regenerate cache for the single tenant (ourselves)
    FlushSelf {},
    /// Need to regenerate cache for multiple tenants due to shared data update (template shop etc.)
    /// 
    /// Needed as an update to a template on the template shop may affect multiple guilds
    FlushOthers {
        others: Vec<Id>,
    },
}

/// WorkerCacheData stores cache related data for templates and associated data (like key expiries)
/// 
/// NOTE: WorkerCache (WIP) will use WorkerCacheData on top of WorkerVmManager to allow for cache regeneration etc
/// 
/// WorkerCacheData is explicitly thread safe (and is one of the few parts of workers that is thread safe)
#[derive(Clone)]
pub struct WorkerCacheData {
    db: WorkerDB,
    templates: CacheEntry<Id, ArcVec<Template>>, // Maps template names to their associated keys
    deferred_cache_regens: CacheEntry<Id, DeferredCacheRegenerationMode>, // Maps id to deferred cache regeneration mode
}

impl WorkerCacheData {
    /// Creates a new WorkerCacheData instance
    ///
    /// This will also set up the initial cache from the database
    pub async fn new(db: WorkerDB) -> Result<Self, crate::Error> {
        let data = Self {
            db,
            templates: CacheEntry::new(),
            deferred_cache_regens: CacheEntry::new(),
        };

        // Setup initial cache from database
        data.setup().await?;

        Ok(data)
    }

    /// Sets up the initial template and key expiry cache
    pub async fn setup(&self) -> Result<(), crate::Error> {
        self.populate_templates().await?;
        Ok(())
    }

    /// Returns the underlying WorkerDB
    pub fn db(&self) -> &WorkerDB {
        &self.db
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

    /// Populates the templates cache from the database
    pub async fn populate_templates(&self) -> Result<(), crate::Error> {
        let templates = self.db.get_templates().await?;

        for (id, templates) in templates {
            self.templates.insert(id, templates).await;
        }

        Ok(())
    }

    /// Gets all templates for a tenant from the database and stores them in the cache
    /// replacing the existing templates in cache
    /// 
    /// Note that this method will *NOT* regenerate Lua VMs
    pub async fn repopulate_templates_for(&self, id: Id) -> Result<(), crate::Error> {
        let templates = self.db.get_templates_for(id).await?;

        // Store the templates in the cache
        self.templates.insert(id, templates).await;
        Ok(())
    }

    /// Sets a deferred cache regeneration mode for a tenant
    pub async fn set_deferred_cache_regeneration(&self, id: Id, mode: DeferredCacheRegenerationMode) {
        self.deferred_cache_regens.insert(id, mode).await;
    }

    /// Gets and removes the deferred cache regeneration mode for a tenant
    pub async fn take_deferred_cache_regeneration(&self, id: &Id) -> Option<DeferredCacheRegenerationMode> {
        self.deferred_cache_regens.remove(id).await
    }
}

// Assert that WorkerCacheData is Send + Sync + Clone
const _: () = {
    const fn assert_send_sync_clone<T: Send + Sync + Clone>() {}
    assert_send_sync_clone::<WorkerCacheData>();
};
