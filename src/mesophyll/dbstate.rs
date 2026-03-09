use std::{collections::{HashMap, HashSet}, sync::Arc};
use tokio::sync::RwLock;
use crate::{geese::gkv::GlobalKeyValueDb, mesophyll::dbtypes::TenantState, worker::workervmmanager::Id};
use crate::geese::kv::KeyValueDb;

#[derive(Clone)]
pub struct DbState {
    pool: sqlx::PgPool,
    key_value_db: KeyValueDb,
    global_key_value_db: GlobalKeyValueDb,
    num_workers: usize,
    tenant_state_cache: Arc<RwLock<HashMap<Id, TenantState>>>, // global tenant state cache
}

impl DbState {
    pub async fn new(num_workers: usize, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let mut s = Self {
            key_value_db: KeyValueDb::new(pool.clone()),
            global_key_value_db: GlobalKeyValueDb::new(pool.clone()),
            pool,
            num_workers,
            tenant_state_cache: Arc::new(RwLock::new(HashMap::new())),
        };

        s.tenant_state_cache = Arc::new(RwLock::new(s.get_tenant_state().await?));

        Ok(s)
    }

    /// Returns the number of workers in the pool
    pub fn num_workers(&self) -> usize {
        self.num_workers
    }

    /// Returns the underlying SQLx Postgres pool
    pub fn get_pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Returns the underlying key-value database interface
    pub fn key_value_db(&self) -> &KeyValueDb {
        &self.key_value_db
    }

    /// Returns the underlying global key-value database interface
    pub fn global_key_value_db(&self) -> &GlobalKeyValueDb {
        &self.global_key_value_db
    }

    /// Returns the underlying tenant state cache
    pub async fn tenant_state_cache_for(&self, worker_id: usize) -> HashMap<Id, TenantState> {
        let cache = self.tenant_state_cache.read().await;
        let mut tenant_states_for_worker = HashMap::new();
        for (id, ts) in cache.iter() {
            if id.worker_id(self.num_workers) == worker_id {
                tenant_states_for_worker.insert(*id, ts.clone());
            }
        }
        tenant_states_for_worker
    }

    /// Returns the tenant state(s) for all tenant in the database
    /// 
    /// Should only be called once, on startup, to initialize the tenant state cache
    async fn get_tenant_state(&self) -> Result<HashMap<Id, TenantState>, crate::Error> {
        #[derive(sqlx::FromRow)]
        struct TenantStatePartial {
            events: Vec<String>,
            flags: i32,
            owner_id: String,
            owner_type: String,
        }

        let partials: Vec<TenantStatePartial> =
            sqlx::query_as("SELECT owner_id, owner_type, events, flags FROM tenant_state")
            .fetch_all(&self.pool)
            .await?;

        let mut states = HashMap::new();  
        for partial in partials {
            let Some(id) = Id::from_parts(&partial.owner_type, &partial.owner_id) else {
                continue;
            };
            let state = TenantState {
                events: HashSet::from_iter(partial.events),
                flags: partial.flags,
            };

            states.insert(id, state);
        }

        Ok(states)
    }

    /// Sets the tenant state for a specific tenant and updates the internal cache
    pub async fn set_tenant_state_for(&self, id: Id, state: TenantState) -> Result<(), crate::Error> {
        let events = state.events.iter().collect::<Vec<_>>();
        sqlx::query(
            "INSERT INTO tenant_state (owner_id, owner_type, events, flags) VALUES ($1, $2, $3, $4) ON CONFLICT (owner_id, owner_type) DO UPDATE SET events = EXCLUDED.events, flags = EXCLUDED.flags",
        )
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .bind(&events)
        .bind(&state.flags)
        .execute(&self.pool)
        .await?;

        let mut cache = self.tenant_state_cache.write().await;
        cache.insert(id, state);

        Ok(())
    }
}
