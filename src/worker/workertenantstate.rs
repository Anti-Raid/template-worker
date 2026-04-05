use std::{cell::RefCell, collections::{HashMap, HashSet}, rc::Rc, sync::Arc};

use crate::{geese::tenantstate::{ModFlags, TenantState}, mesophyll::client::MesophyllClient, worker::workervmmanager::{Id, WorkerVmManager}};

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
    pub fn reload_for_tenant(&self, id: Id, tenant_state: &TenantState) -> Result<(), crate::Error> {
        let reload_vm = tenant_state.modflags.contains(ModFlags::BANNED);
        {
            let mut cache = self.tenant_state_cache.borrow_mut();
            cache.insert(id, tenant_state.clone());
        }

        // Drop any bad tenants here 
        if reload_vm {
            self.vm_manager.remove_vm_for(id)?; 
        }

        Ok(())
    }

    /// Gets the tenant state for a specific tenant
    pub fn get_cached_tenant_state_for(&self, id: Id) -> Result<TenantState, crate::Error> {
        let cache = self.tenant_state_cache.borrow();
        match cache.get(&id) {
            Some(state) => Ok(state.clone()),
            None => Ok(TenantState::default())
        }
    }
    /// Returns the set of tenant IDs that have startup events enabled
    pub fn get_startup_event_tenants(&self) -> HashSet<Id> {
        let mut startup_events = HashSet::new();  
        let ts = self.tenant_state_cache.borrow();
        for (id, ts) in ts.iter() {
            // Track startup events
            if ts.events.contains_key("OnStartup") {
                startup_events.insert(*id);
            }
        }
        startup_events
    }
}