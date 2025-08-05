//! # Workers
//! 
//! **Note: Workers are a work in progress and are not yet fully implemented.**
//! 
//! A worker is the base unit for work such as dispatching events to Luau VM's in AntiRaid
//! 
//! There are currently two layer in a worker construct:
//! 
//! - WorkerVMManager: Stores the Luau VM's per guild/user and handles the creation and retrieval of VMs within a worker
//! - WorkerDispatcher: Dispatches events to the Luau VM's in a worker
//! - WorkerCacheData: Caches data such as templates and key expiries for a worker

pub mod workerdispatch;
pub mod workervmmanager;
pub mod workerstate;
pub mod limits;
pub mod vmcontext;
pub mod workercachedata;
pub mod workercache;
pub mod builtins;