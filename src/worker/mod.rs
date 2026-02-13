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
//! - WorkerDispatcher: Dispatches events to the Luau VM's in a worker, handles resume keys and deferred cache regeneration as well
//! - WorkerVmManager: Stores the Luau VM's per guild/user and handles the creation and retrieval of VMs within a worker
//! - WorkerState: Provides state and database related code to the worker system
//! - VMContext + vmdatastores.rs: Provides a TemplateConextProvider for the AntiRaid Khronos Luau Runtime [internal]
//! - Worker: Encapsulates a WorkerVmManager, WorkerDispatcher, and WorkerCache for easy use
//! - WorkerThread: Allows spinning up a worker thread that can be used to run the worker system
//! - WorkerLike: While not a proper structure, WorkerLike defines the basic needs for a Worker unit
//! - WorkerThreadPool: Provides a thread pool for workers based on Discord's sharding formulapub mod, allowing for multiple worker threads to be spawned and used concurrently

pub mod workerdispatch;
pub mod workervmmanager;
pub mod workerstate;
pub mod limits;
pub mod vmcontext;
pub mod builtins;
pub mod worker;
pub mod workerthread;
pub mod workerlike;
pub mod workerdb;

pub mod workerprocesshandle; // TODO: Replace with mesophyll
pub mod workerpool;