use std::{cell::RefCell, collections::{HashMap, HashSet, hash_map::Entry}, rc::Rc, sync::{Arc, LazyLock}};

use crate::{geese::tenantstate::{ModFlags, TenantState}, mesophyll::client::MesophyllClient, worker::workervmmanager::{Id, WorkerVmManager}};

static DEFAULT_TENANT_STATE: LazyLock<TenantState> = LazyLock::new(|| {
    let ts = TenantState {
        events: {
            let mut set = HashSet::new();
            set.insert("INTERACTION_CREATE".to_string());
            set.insert("WebGetSettings".to_string());
            set.insert("WebExecuteSetting".to_string());
            set
        },
        flags: 0,
        modflags: ModFlags::empty()
    };

    ts
});

#[derive(Clone)]
pub struct WorkerTenantState {
    vm_manager: WorkerVmManager,
    tenant_state_cache: Rc<RefCell<HashMap<Id, TenantState>>>, // Maps tenant IDs to their states
}

impl WorkerTenantState {
    pub async fn new(mesophyll_client: Arc<MesophyllClient>, vm_manager: WorkerVmManager) -> Result<Self, crate::Error> {
        // Initialize the tenant state cache with the current tenant states from the database
        //
        // The tenant state cache acts as a routing table
        let t_states = mesophyll_client.list_tenant_states().await?;
        Ok(Self {
            vm_manager,
            tenant_state_cache: Rc::new(RefCell::new(t_states))
        })
    }

    /// Reloads the tenant state cache for a worker
    pub fn reload_for_tenant(&self, id: Id, events: Vec<String>, flags: i32, modflags: Option<ModFlags>) -> Result<(), crate::Error> {
        let events_set = HashSet::from_iter(events);

        {
            let mut cache = self.tenant_state_cache.borrow_mut();
            match cache.entry(id) {
                Entry::Occupied(mut e) => {
                    let old_modflags = if let Some(modflags) = modflags { modflags } else { e.get().modflags };
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
                        modflags: if let Some(modflags) = modflags { modflags } else { ModFlags::empty() }
                    });
                }
            };
        }

        // Drop any bad tenants here if modflags is set
        if let Some(modflags) = modflags {
            if modflags.contains(ModFlags::BANNED) {
                self.vm_manager.remove_vm_for(id)?;
            }
        }

        Ok(())
    }

    /// Gets the tenant state for a specific tenant
    pub fn get_cached_tenant_state_for(&self, id: Id) -> Result<TenantState, crate::Error> {
        let cache = self.tenant_state_cache.borrow();
        match cache.get(&id) {
            Some(state) => Ok(state.clone()),
            None => {
                // Return the default tenant state if not found in cache
                Ok(DEFAULT_TENANT_STATE.clone())
            }
        }
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
}