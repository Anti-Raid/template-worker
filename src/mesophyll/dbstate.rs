use crate::geese::{state::StateDb, tenantstate::TenantStateDb};

#[derive(Clone)]
pub struct DbState {
    pool: sqlx::PgPool,
    tenant_state_db: TenantStateDb,
    state_db: StateDb,
    num_workers: usize,
}

impl DbState {
    pub async fn new(num_workers: usize, pool: sqlx::PgPool) -> Result<Self, crate::Error> {
        let s = Self {
            tenant_state_db: TenantStateDb::new(pool.clone()),
            state_db: StateDb::new(pool.clone()),
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

    /// Returns the underlying tenant state db
    pub fn tenant_state_db(&self) -> &TenantStateDb {
        &self.tenant_state_db
    }

    /// Returns the underlying key-value database interface
    pub fn state_db(&self) -> &StateDb {
        &self.state_db
    }
}
