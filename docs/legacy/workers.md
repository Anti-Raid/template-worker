# Workers

### Note: Old System Note

Older versions of AntiRaid used a much more entangled/complex system for dispatching events to templates and managing state/lifecycles/key expiries/key resumption etc. See [here](./legacy-arch) for more information.

### Warning: Advanced/Internal Details

This document contains advanced/internal details that are not necessary for most users of AntiRaid to understand. If you are just looking to use AntiRaid, you can skip this document.

Knowledge of Rust and Lua (specifically Luau) is required to understand this document. If you do not understand (at minimum) ``Rc/RefCell/Arc/Mutex/RwLock``, traits, generics, enums, pattern matching, ``extern "C"`` and ``extern "C-unwind"``, Result/Option, closures/move, async/await (``Future``), atomic operations via ``std::async::atomic``, lifetimes (especially `'static` lifetime), auto-traits like ``Send/Sync``, basic traits like ``Clone`` and ``Drop`` and basic Rust constructs like ``Box<T>`` and ``Box<dyn Trait>``, then learn those first before continuing.

## Overview

A worker is the base unit for work such as dispatching events to Luau VM's in AntiRaid. Paired with a ``WorkerLike`` distribution mechanism (``WorkerThread`` for single thread ``WorkerLike`` topology, ``WorkerProcessHandle`` for a single process ``WorkerLike`` topology or ``WorkerPool`` for making a pool of ``WorkerLike + Poolable``'s into a larger ``WorkerLike`` topology)

There are currently multiple layers in a worker construct:

- ``builtins.rs``: Provides an embedded VFS containing the builtins of AntiRaid (core Discord commands etc, core pure Luau libraries etc.)
- ``KeyExpiry``: Provides a task that handles key expiries and dispatches them to the worker system
- ``KeyExpiryChannel``: Provides a channel for key expiries to be sent to the worker system while supporting on-demand repopulation on key expiry changes
- ``limits.rs``: Provides limits for the worker system such as max template size, max key/value size along with GCRA-based ratelimiters for actions that can be performed by templates.
- ``staticref.rs``: Experimental optimization to avoid reference counting for long lived objects by making them a ``&static T`` instead of an ``Arc<T>`` or ``Rc<T>``
- ``Template``: Represents a template that can be executed in a worker along with its data and functions to fetch them.
- ``VMContext`` (and ``vmdatastores.rs``): Provides a TemplateConextProvider for the AntiRaid Khronos Runtime 
- ``Worker``: Encapsulates a ``WorkerVmManager``, ``WorkerDispatcher``, ``WorkerDB``, ``WorkerFilter``, ``KeyExpiryChannel`` and ``WorkerKeyExpiry`` into a single struct for easy use elsewhere
- ``WorkerCacheData``: Caches data such as templates and key expiries for a worker
- ``WorkerDB``: Provides database related code to the worker system
- ``WorkerDispatcher``: Dispatches events to the Luau VM's in a worker along with handling dispatch lifecycle related systems like resume keys and deferred cache regeneration as well
- ``WorkerFilter``: Allows filtering automatic event dispatches within a worker based on tenant ID and worker ID
- ``WorkerLike``: While not a proper structure, WorkerLike defines the basic needs for a Worker unit/topology
- ``WorkerPool``: Provides a pool for pooling multiple ``WorkerLike + Poolable``'s into a larger ``WorkerLike`` topology with automatic routing based on tenant ID and Discord's sharding formula (or other deterministic ID based routing mechanisms)
- ``Poolable``: While not a proper structure, Poolable defines the basic needs for a WorkerLike unit to be pooled into a larger WorkerLike topology via ``WorkerPool``. ``Poolable`` is defined within ``workerpool.rs`` itself along with ``WorkerPool``
- ``workerprocesscomm.rs``: Defines basic communication primitives and traits for allowing communication between master process and a worker so future changes to AntiRaid can add more process communication protocols (besides the current HTTP/2 API) if needed/desired.
- ``WorkerProcessCommHttp2``: Provides an HTTP/2 based communication protocol for communicating between master process and a worker process. The internal API will communicate over HTTP/2 using a randomized port and randomized token to avoid spoofing and is not intended to be used outside of inter-worker communication (e.g. no AntiRaid tooling should directly use this API and should instead use the ``WorkerProcessCommServer`` trait from the master process directly and then expose it via a internal API in the master process if needed)
- ``WorkerProcessHandle``: A ``WorkerLike`` topology that spins up a worker process that communicates with the master process using the given communication backend (currently only ``WorkerProcessCommHttp2`` is supported)
- ``WorkerState``: Stores any global worker resources such as the database connection pool, serenity http client, object storage client and info about the AntiRaid bot user itself
- ``WorkerThread``: A ``WorkerLike`` topology that spins up a worker thread and communicates with it using a tokio mpsc channel (with optional oneshot callback channels for request/response style messages). Note that the ``WorkerThread`` topology is fully isolated from the rest of the worker system and hence does not have the entaglement issues the previous architecture had.
- ``WorkerVmManager``: Stores the Luau VM's per guild/user and handles the creation and retrieval of VMs within a worker

Of note, notice that all of the above layers do one single thing (and does it well) and also only stores the other layers they need as struct fields. This is the guiding principle of AntiRaid's 'worker architecture'
