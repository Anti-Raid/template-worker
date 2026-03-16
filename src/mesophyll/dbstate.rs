use crate::{geese::{gkv::GlobalKeyValueDb, tenantstate::TenantStateDb}};
use crate::geese::kv::KeyValueDb;

#[derive(Clone)]
pub struct DbState {
    pool: sqlx::PgPool,
    key_value_db: KeyValueDb,
    global_key_value_db: GlobalKeyValueDb,
    tenant_state_db: TenantStateDb,
    num_workers: usize,
}

impl DbState {
    pub async fn new(num_workers: usize, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let s = Self {
            key_value_db: KeyValueDb::new(pool.clone()),
            global_key_value_db: GlobalKeyValueDb::new(pool.clone()),
            tenant_state_db: TenantStateDb::new(pool.clone()),
            pool,
            num_workers,
        };

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

    /// Returns the underlying tenant state db
    pub fn tenant_state_db(&self) -> &TenantStateDb {
        &self.tenant_state_db
    }
}
