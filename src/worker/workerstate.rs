use std::{borrow::Cow, cell::RefCell, collections::{HashMap, HashSet}, rc::Rc, sync::{Arc, LazyLock}};
use serde_json::Value;

use crate::worker::{workerdb::WorkerDB, workervmmanager::Id};

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
    pub object_store: Arc<crate::objectstore::ObjectStore>,
    pub current_user: Arc<serenity::all::CurrentUser>,
    pub mesophyll_db: Arc<WorkerDB>
}

impl CreateWorkerState {
    /// Creates a new CreateWorkerState with the given serenity context, reqwest client, object store, and database pool
    pub fn new(
        serenity_http: Arc<serenity::http::Http>,
        reqwest_client: reqwest::Client,
        object_store: Arc<crate::objectstore::ObjectStore>,
        current_user: Arc<serenity::all::CurrentUser>,
        mesophyll_db: Arc<WorkerDB>
    ) -> Self {
        Self {
            serenity_http,
            reqwest_client,
            object_store,
            current_user,
            mesophyll_db
        }
    }
}

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct WorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub reqwest_client: reqwest::Client,
    pub object_store: Arc<crate::objectstore::ObjectStore>,
    pub mesophyll_db: Arc<WorkerDB>,
    pub current_user: Arc<serenity::all::CurrentUser>,
    tenant_state_cache: Rc<RefCell<HashMap<Id, TenantState>>>, // Maps tenant IDs to their states
    startup_events: Rc<RefCell<HashSet<Id>>>, // Tracks which tenants have had their startup events fired
}

impl WorkerState {
    /// Creates a new WorkerState with the given serenity context, reqwest client, object store, and database pool
    pub async fn new(cws: CreateWorkerState) -> Result<Self, crate::Error> {
        let tenant_state_cache = Rc::new(RefCell::new(HashMap::new()));
        let startup_events = Rc::new(RefCell::new(HashSet::new()));
        let s = Self {
            serenity_http: cws.serenity_http,
            reqwest_client: cws.reqwest_client,
            object_store: cws.object_store,
            mesophyll_db: cws.mesophyll_db,
            current_user: cws.current_user,
            tenant_state_cache,
            startup_events,
        };

        // Initialize the tenant state cache with the current tenant states from the database
        //
        // The tenant state cache acts as a routing table
        let (t_states, startup_events) = s.get_tenant_state().await?;
        *s.tenant_state_cache.borrow_mut() = t_states;
        *s.startup_events.borrow_mut() = startup_events;

        Ok(s)
    }

    /// Returns the tenant state(s) for all guilds in the database as well as a set of guild IDs that have startup events enabled
    /// 
    /// Should only be called once, on startup, to initialize the tenant state cache
    async fn get_tenant_state(&self) -> Result<(HashMap<Id, TenantState>, HashSet<Id>), crate::Error> {
        let states = self.mesophyll_db.list_tenant_states().await?;

        let mut startup_events = HashSet::new();  
        for (id, ts) in states.iter() {
            // Track startup events
            if ts.events.contains(&"OnStartup".to_string()) {
                startup_events.insert(id.clone());
            }
        }

        Ok((states, startup_events))
    }

    /// Returns the set of tenant IDs that have startup events enabled
    pub fn get_startup_event_tenants(&self) -> Result<std::cell::Ref<'_, HashSet<Id>>, crate::Error> {
        Ok(self.startup_events.try_borrow()?)
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

        // Update startup events tracking
        {
            let mut startup_events = self.startup_events.borrow_mut();
            if state.events.contains(&"OnStartup".to_string()) {
                startup_events.insert(id);
            } else {
                startup_events.remove(&id);
            }
        }

        // Update the cache
        {
            let mut cache = self.tenant_state_cache.borrow_mut();
            cache.insert(id, state);
        }

        Ok(())
    }
}
