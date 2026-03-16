use std::{borrow::Cow, cell::RefCell, collections::{HashMap, HashSet, hash_map::Entry}, rc::Rc, sync::{Arc, LazyLock}};
use crate::{geese::{objectstore::ObjectStore, stratum::Stratum, tenantstate::{ModFlags, TenantState}}, mesophyll::client::MesophyllClient, worker::workervmmanager::Id};

pub static DEFAULT_TENANT_STATE: LazyLock<TenantState> = LazyLock::new(|| TenantState {
    events: {
        let mut set = HashSet::new();
        set.insert("INTERACTION_CREATE".to_string());
        set.insert("WebGetSettings".to_string());
        set.insert("WebExecuteSetting".to_string());
        set
    },
    flags: 0,
    modflags: ModFlags::empty()
});

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct CreateWorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub object_store: Arc<ObjectStore>,
    pub current_user: Arc<serenity::all::CurrentUser>,
    pub mesophyll_client: Arc<MesophyllClient>,
    pub stratum: Stratum,
    pub worker_print: bool,
}

impl CreateWorkerState {
    /// Creates a new CreateWorkerState with the given serenity context, reqwest client, object store, and database pool
    pub fn new(
        serenity_http: Arc<serenity::http::Http>,
        object_store: Arc<ObjectStore>,
        current_user: Arc<serenity::all::CurrentUser>,
        mesophyll_client: Arc<MesophyllClient>,
        stratum: Stratum,
        worker_print: bool
    ) -> Self {
        Self {
            serenity_http,
            object_store,
            current_user,
            mesophyll_client,
            stratum,
            worker_print
        }
    }
}

#[derive(Clone)]
/// Represents the state of the worker, which includes the serenity context, reqwest client, object store, and database pool
pub struct WorkerState {
    pub serenity_http: Arc<serenity::http::Http>,
    pub object_store: Arc<ObjectStore>,
    pub mesophyll_client: Arc<MesophyllClient>,
    pub current_user: Arc<serenity::all::CurrentUser>,
    pub stratum: Stratum,
    pub worker_print: bool,
    tenant_state_cache: Rc<RefCell<HashMap<Id, TenantState>>>, // Maps tenant IDs to their states
}

impl WorkerState {
    /// Creates a new WorkerState with the given serenity context, reqwest client, object store, and database pool
    pub async fn new(cws: CreateWorkerState) -> Result<Self, crate::Error> {
        let tenant_state_cache = Rc::new(RefCell::new(HashMap::new()));
        let s = Self {
            serenity_http: cws.serenity_http,
            object_store: cws.object_store,
            mesophyll_client: cws.mesophyll_client,
            current_user: cws.current_user,
            stratum: cws.stratum,
            worker_print: cws.worker_print,
            tenant_state_cache,
        };

        // Initialize the tenant state cache with the current tenant states from the database
        //
        // The tenant state cache acts as a routing table
        let t_states = s.mesophyll_client.list_tenant_states().await?;
        *s.tenant_state_cache.borrow_mut() = t_states;

        Ok(s)
    }

    /// Returns the set of tenant IDs that have startup events enabled
    pub fn get_startup_event_tenants(&self) -> HashSet<Id> {
        let mut startup_events = HashSet::new();  
        let ts = self.tenant_state_cache.borrow();
        for (id, ts) in ts.iter() {
            // Track startup events
            if ts.events.contains("OnStartup") {
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
    pub async fn set_tenant_state_for(&self, id: Id, events: Vec<String>, flags: i32) -> Result<(), crate::Error> {
        self.mesophyll_client.set_tenant_state_for(id, events.clone(), flags).await?;
        let events_set = HashSet::from_iter(events);

        // Update the cache
        {
            let mut cache = self.tenant_state_cache.borrow_mut();
            match cache.entry(id) {
                Entry::Occupied(mut e) => {
                    let old_modflags = e.get().modflags;
                    e.insert(TenantState {
                        events: events_set,
                        flags,
                        modflags: old_modflags,
                    }); 
                }
                Entry::Vacant(vc) => {
                    vc.insert(TenantState { 
                        events: events_set, 
                        flags, 
                        modflags: ModFlags::empty()
                    });
                }
            };
        }

        Ok(())
    }
}
