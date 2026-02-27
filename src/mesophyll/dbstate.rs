use std::{collections::{HashMap, HashSet}, sync::Arc};
use tokio::sync::RwLock;
use crate::{mesophyll::dbtypes::{CreateGlobalKv, GlobalKv, GlobalKvData, PartialGlobalKv}, worker::{workerstate::TenantState, workervmmanager::Id}};
use crate::geese::kv::KeyValueDb;

#[derive(Clone)]
pub struct DbState {
    pool: sqlx::PgPool,
    key_value_db: KeyValueDb,
    num_workers: usize,
    tenant_state_cache: Arc<RwLock<HashMap<Id, TenantState>>>, // global tenant state cache
    purchased_cache: Arc<RwLock<HashSet<(String, Id)>>>, // cache of purchased global kvs (key, tenant id)
}

impl DbState {
    pub async fn new(num_workers: usize, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let mut s = Self {
            key_value_db: KeyValueDb::new(pool.clone()),
            pool,
            num_workers,
            tenant_state_cache: Arc::new(RwLock::new(HashMap::new())),
            purchased_cache: Arc::new(RwLock::new(HashSet::new())),
        };

        s.tenant_state_cache = Arc::new(RwLock::new(s.get_tenant_state().await?));

        Ok(s)
    }

    /// Returns the underlying SQLx Postgres pool
    pub fn get_pool(&self) -> &sqlx::PgPool {
        &self.pool
    }

    /// Returns the underlying key-value database interface
    pub fn key_value_db(&self) -> &KeyValueDb {
        &self.key_value_db
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
            data: serde_json::Value,
            owner_id: String,
            owner_type: String,
        }

        let partials: Vec<TenantStatePartial> =
            sqlx::query_as("SELECT owner_id, owner_type, events, data FROM tenant_state")
            .fetch_all(&self.pool)
            .await?;

        let mut states = HashMap::new();  
        for partial in partials {
            let Some(id) = Id::from_parts(&partial.owner_type, &partial.owner_id) else {
                continue;
            };
            let state = TenantState {
                events: HashSet::from_iter(partial.events),
                data: partial.data,
            };

            states.insert(id, state);
        }

        Ok(states)
    }

    /// Sets the tenant state for a specific tenant and updates the internal cache
    pub async fn set_tenant_state_for(&self, id: Id, state: TenantState) -> Result<(), crate::Error> {
        let events = state.events.iter().collect::<Vec<_>>();
        sqlx::query(
            "INSERT INTO tenant_state (owner_id, owner_type, events, data) VALUES ($1, $2, $3, $4) ON CONFLICT (owner_id, owner_type) DO UPDATE SET events = EXCLUDED.events, data = EXCLUDED.data",
        )
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .bind(&events)
        .bind(&state.data)
        .execute(&self.pool)
        .await?;

        let mut cache = self.tenant_state_cache.write().await;
        cache.insert(id, state);

        Ok(())
    }

    // TODO: Actually implement this
    async fn global_kv_is_purchased(&self, key: String, tid: Id) -> bool {
        let cache = self.purchased_cache.read().await;
        cache.contains(&(key, tid))
    }

    // TODO: Actually implement this
    async fn global_kv_to_url(&self, key: &str) -> String {
        // TODO: Replace with actual purchase URL generation logic
        format!("https://example.com/purchase/{key}")
    }

    pub async fn global_kv_find(&self, scope: String, query: String) -> Result<Vec<PartialGlobalKv>, crate::Error> {
        let items: Vec<PartialGlobalKv> = if query == "%%" {
            sqlx::query_as(
                "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved'"
            )
            .bind(scope)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved' AND key LIKE $2"
            )
            .bind(scope)
            .bind(query)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(items)
    }
    
    pub async fn global_kv_get(&self, key: String, version: i32, scope: String, id: Option<Id>) -> Result<Option<GlobalKv>, crate::Error> {
        let item: Option<GlobalKv> = sqlx::query_as(
            "SELECT key, version, owner_id, owner_type, short, long, data, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE review_state = 'approved' AND key = $1 AND version = $2 AND scope = $3",
        )
        .bind(&key)
        .bind(version)
        .bind(scope)
        .fetch_optional(&self.pool)
        .await?;

        let Some(mut gkv) = item else {
            return Ok(None);
        };

        // Drop data immediately here to ensure it is not leaked
        let data = std::mem::replace(&mut gkv.raw_data, serde_json::Value::Null);

        if gkv.partial.price.is_some() {
            match id {
                Some(tid) => {
                    // Check if purchased
                    let is_purchased = self.global_kv_is_purchased(key, tid).await;
                    if !is_purchased {
                        gkv.data = GlobalKvData::PurchaseRequired {
                            purchase_url: self.global_kv_to_url(&gkv.partial.key).await,
                        };
                        return Ok(Some(gkv));
                    }
                }
                None => {
                    // No tenant ID provided, cannot verify purchase
                    gkv.data = GlobalKvData::PurchaseRequired {
                        purchase_url: self.global_kv_to_url(&gkv.partial.key).await,
                    };
                    return Ok(Some(gkv));
                }
            }
        }

        let opaque = gkv.partial.price.is_some() || !gkv.partial.public_data;
        gkv.data = GlobalKvData::Value { data, opaque };

        Ok(Some(gkv))
    }

    pub async fn global_kv_create(&self, id: Id, gkv: CreateGlobalKv) -> Result<(), crate::Error> {
        // Validate key
        //
        // Rules:
        // 1. Between 3 and 64 characters long
        // 2. May not start or end with a dot (.)
        // 3. May only contain (ASCII) alphanumeric characters, dots (.), dashes (-), and underscores (_)
        if gkv.key.len() < 3 || gkv.key.len() > 64 {
            return Err("keys must be between 3 and 64 characters long".into());
        }
        if gkv.key.starts_with('.') || gkv.key.ends_with('.') {
            return Err("keys may not start or end with a dot".into());
        }
        if !gkv.key.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
            return Err("keys may only contain alphanumeric characters, dots, dashes, and underscores".into());
        }

        let inserted = sqlx::query(
            "INSERT INTO global_kv (key, version, owner_id, owner_type, short, long, public_metadata, public_data, scope, data) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) ON CONFLICT (key, version, scope) DO NOTHING",
        )
        .bind(&gkv.key)
        .bind(gkv.version)
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .bind(&gkv.short)
        .bind(&gkv.long)
        .bind(&gkv.public_metadata)
        .bind(gkv.public_data)
        .bind(&gkv.scope)
        .bind(&gkv.data)
        .execute(&self.pool)
        .await?;

        if inserted.rows_affected() == 0 {
            return Err("Global KV with the same key, version, and scope already exists".into());
        }

        Ok(())
    }

    pub async fn global_kv_delete(&self, id: Id, key: String, version: i32, scope: String) -> Result<(), crate::Error> {
        let res = sqlx::query(
        "DELETE FROM global_kv WHERE key = $1 AND version = $2 AND scope = $3 AND owner_id = $4 AND owner_type = $5",
        )
        .bind(key)
        .bind(version)
        .bind(scope)
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .execute(&self.pool)
        .await?;

        if res.rows_affected() == 0 {
            return Err("No matching Global KV found to delete or insufficient permissions".into());
        }

        Ok(())
    }
}
