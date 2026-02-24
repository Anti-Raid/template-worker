use khronos_runtime::utils::khronos_value::KhronosValue;

use crate::{mesophyll::{client::MesophyllDbClient, dbtypes::{CreateGlobalKv, PartialGlobalKv, GlobalKv, SerdeKvRecord}, dbstate::DbState}, worker::{workerstate::TenantState, workervmmanager::Id}};

/// An abstraction over the database access method for worker state
pub enum WorkerDB {
    Direct(DbState),
    Mesophyll(MesophyllDbClient)
}

impl WorkerDB {
    pub fn new_direct(db_state: DbState) -> Self {
        WorkerDB::Direct(db_state)
    }

    pub fn new_mesophyll(client: MesophyllDbClient) -> Self {
        WorkerDB::Mesophyll(client)
    }

    pub async fn set_tenant_state_for(&self, id: Id, state: &TenantState) -> Result<(), crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.set_tenant_state_for(id, state.clone()).await,
            WorkerDB::Mesophyll(c) => c.set_tenant_state_for(id, state).await,
        }
    }

    pub async fn kv_get(&self, id: Id, scopes: Vec<String>, key: String) -> Result<Option<SerdeKvRecord>, crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.key_value_db().kv_get(id, scopes, key).await,
            WorkerDB::Mesophyll(c) => c.kv_get(id, scopes, key).await,
        }
    }

    pub async fn kv_list_scopes(&self, id: Id) -> Result<Vec<String>, crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.key_value_db().kv_list_scopes(id).await,
            WorkerDB::Mesophyll(c) => c.kv_list_scopes(id).await,
        }
    }

    pub async fn kv_set(&self, id: Id, scopes: Vec<String>, key: String, value: KhronosValue) -> Result<(), crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.key_value_db().kv_set(id, scopes, key, value).await,
            WorkerDB::Mesophyll(c) => c.kv_set(id, scopes, key, value).await,
        }
    }

    pub async fn kv_delete(&self, id: Id, scopes: Vec<String>, key: String) -> Result<(), crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.key_value_db().kv_delete(id, scopes, key).await,
            WorkerDB::Mesophyll(c) => c.kv_delete(id, scopes, key).await,
        }
    }

    pub async fn kv_find(&self, id: Id, scopes: Vec<String>, prefix: String) -> Result<Vec<SerdeKvRecord>, crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.key_value_db().kv_find(id, scopes, prefix).await,
            WorkerDB::Mesophyll(c) => c.kv_find(id, scopes, prefix).await,
        }
    }

    pub async fn global_kv_find(&self, scope: String, query: String) -> Result<Vec<PartialGlobalKv>, crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.global_kv_find(scope, query).await,
            WorkerDB::Mesophyll(c) => c.global_kv_find(scope, query).await,
        }
    }

    pub async fn global_kv_get(&self, key: String, version: i32, scope: String, id: Option<Id>) -> Result<Option<GlobalKv>, crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.global_kv_get(key, version, scope, id).await,
            WorkerDB::Mesophyll(c) => c.global_kv_get(key, version, scope, id).await,
        }
    }

    pub async fn global_kv_create(&self, id: Id, gkv: CreateGlobalKv) -> Result<(), crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.global_kv_create(id, gkv).await,
            WorkerDB::Mesophyll(c) => c.global_kv_create(id, gkv).await,
        }
    }

    pub async fn global_kv_delete(&self, id: Id, key: String, version: i32, scope: String) -> Result<(), crate::Error> {
        match self {
            WorkerDB::Direct(d) => d.global_kv_delete(id, key, version, scope).await,
            WorkerDB::Mesophyll(c) => c.global_kv_delete(id, key, version, scope).await,
        }
    }
}