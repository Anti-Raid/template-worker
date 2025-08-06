//! # Workers
//! 
//! **Note: Workers are a work in progress and are not yet fully implemented.**
//! 
//! A worker is the base unit for work such as dispatching events to Luau VM's in AntiRaid. 
//! 
//! Note that workers must be paired with a distribution mechanism such as a thread or process pool
//! 
//! There are currently multiple layers in a worker construct:
//! 
//! - WorkerVmManager: Stores the Luau VM's per guild/user and handles the creation and retrieval of VMs within a worker
//! - WorkerDispatcher: Dispatches events to the Luau VM's in a worker, handles resume keys and deferred cache regeneration as well
//! - WorkerCacheData: Caches data such as templates and key expiries for a worker
//! - VMContext + vmdatastores.rs: Provides a TemplateConextProvider for the AntiRaid Khronos Luau Runtime [internal]
//! - Worker: Encapsulates a WorkerVmManager, WorkerDispatcher, and WorkerCache for easy use
//! - WorkerDB: Provides database related code to the worker system
//! - WorkerThread: Allows spinning up a worker thread that can be used to run the worker system
//! - Template: Represents a template that can be executed in a worker

pub mod workerdispatch;
pub mod workervmmanager;
pub mod workerstate;
pub mod limits;
pub mod vmcontext;
pub mod vmdatastores;
pub mod workercachedata;
pub mod builtins;
pub mod worker;
pub mod workerdb;
pub mod workerthread;
pub mod template;