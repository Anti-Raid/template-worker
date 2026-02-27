use std::{borrow::Cow, cell::RefCell, collections::{HashMap, HashSet}, rc::Rc, sync::{Arc, LazyLock}};
use serde_json::Value;

use crate::{geese::sandwich::Sandwich, geese::objectstore::ObjectStore, worker::{workerdb::WorkerDB, workervmmanager::Id}};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct TenantState {
    pub events: HashSet<String>,
    pub data: Value
}

static DEFAULT_TENANT_STATE: LazyLock<TenantState> = LazyLock::new(|| TenantState {
    events: {
        let mut set = HashSet::new();
        set.insert("INTERACTION_CREATE".to_string());
        set.insert("WebGetSettings".to_string());
        set.insert("WebExecuteSetting".to_string());
        set
    },
    data: Value::Object(serde_json::Map::new()),
});

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct CreateWorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub reqwest_client: reqwest::Client,
    pub object_store: Arc<ObjectStore>,
    pub current_user: Arc<serenity::all::CurrentUser>,
    pub mesophyll_db: Arc<WorkerDB>,
    pub sandwich: Sandwich,
    pub worker_print: bool,
}

impl CreateWorkerState {
    /// Creates a new CreateWorkerState with the given serenity context, reqwest client, object store, and database pool
    pub fn new(
        serenity_http: Arc<serenity::http::Http>,
        reqwest_client: reqwest::Client,
        object_store: Arc<ObjectStore>,
        current_user: Arc<serenity::all::CurrentUser>,
        mesophyll_db: Arc<WorkerDB>,
        sandwich: Sandwich,
        worker_print: bool
    ) -> Self {
        Self {
            serenity_http,
            reqwest_client,
            object_store,
            current_user,
            mesophyll_db,
            sandwich,
            worker_print
        }
    }
}

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct WorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub _reqwest_client: reqwest::Client,
    pub object_store: Arc<ObjectStore>,
    pub mesophyll_db: Arc<WorkerDB>,
    pub current_user: Arc<serenity::all::CurrentUser>,
    pub sandwich: Sandwich,
    pub worker_print: bool,
    tenant_state_cache: Rc<RefCell<HashMap<Id, TenantState>>>, // Maps tenant IDs to their states
}

impl WorkerState {
    /// Creates a new WorkerState with the given serenity context, reqwest client, object store, and database pool
    pub async fn new(cws: CreateWorkerState, worker_id: usize) -> Result<Self, crate::Error> {
        let tenant_state_cache = Rc::new(RefCell::new(HashMap::new()));
        let s = Self {
            serenity_http: cws.serenity_http,
            _reqwest_client: cws.reqwest_client,
            object_store: cws.object_store,
            mesophyll_db: cws.mesophyll_db,
            current_user: cws.current_user,
            sandwich: cws.sandwich,
            worker_print: cws.worker_print,
            tenant_state_cache,
        };

        // Initialize the tenant state cache with the current tenant states from the database
        //
        // The tenant state cache acts as a routing table
        let t_states = match &*s.mesophyll_db {
            WorkerDB::Direct(ref db) => db.tenant_state_cache_for(worker_id).await,
            WorkerDB::Mesophyll(ref client) => client.list_tenant_states().await?,
        };
        *s.tenant_state_cache.borrow_mut() = t_states;

        Ok(s)
    }

    /// Returns the set of tenant IDs that have startup events enabled
    pub fn get_startup_event_tenants(&self) -> HashSet<Id> {
        let mut startup_events = HashSet::new();  
        let ts = self.tenant_state_cache.borrow();
        for (id, ts) in ts.iter() {
            // Track startup events
            if ts.events.contains(&"OnStartup".to_string()) {
                startup_events.insert(*id);
            }
        }
        startup_events
    }

    /// Gets the tenant state for a specific tenant
    pub fn get_cached_tenant_state_for<'a>(&'a self, id: Id) -> Result<Cow<'a, TenantState>, crate::Error> {
        let cache = self.tenant_state_cache.borrow();
        match cache.get(&id) {
            Some(state) => Ok(Cow::Owned(state.clone())),
            None => {
                // Return the default tenant state if not found in cache
                Ok(Cow::Borrowed(&*DEFAULT_TENANT_STATE))
            }
        }
    }

    /// Sets the tenant state for a specific tenant
    pub async fn set_tenant_state_for(&self, id: Id, state: TenantState) -> Result<(), crate::Error> {
        self.mesophyll_db.set_tenant_state_for(id, &state).await?;

        // Update the cache
        {
            let mut cache = self.tenant_state_cache.borrow_mut();
            cache.insert(id, state);
        }

        Ok(())
    }
}
