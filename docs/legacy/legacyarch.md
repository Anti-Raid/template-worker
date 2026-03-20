# Legacy Architecture

Older versions of AntiRaid used a much more entangled/complex system for dispatching events to templates and managing state/lifecycles/key expiries/key resumption etc. To avoid repeating the mistakes of the past, the below is a summary of how the old system worked:

In the older system a VM distribution strategy defines how guild VMs should be distributed out with two main strategies (later one): thread per guild (which got removed later on) and threadpool. With thread per guild, each guild was given its own thread and with threadpool, a set of guilds share a thread. When either strategy encounters a panic, a per thread panic hook was triggered/fired to clean up all current guild handles so future dispatches to a guild start a fresh new guild vm on a different (non-panicking) thread.

A vm distribution strategy performed any setup needed (to init guild state, the tokio runtime and the khronos runtime). It then ran a loop listening on a request channel for `LuaVmAction` messages. When a action was recieved with its (optional) associated callback channel, the `handle` method of `LuaVmAction` was called which then handled the execution and sent back the results. To simplify things, a `ThreadEntry` structure was created which provided a generic abstract implementation that does all of this work in a way that could (in theory) be used by any VM distribution strategy.

While this sounds 'simple', the actual implementation was quite entangled. Nearly everything was a mess of interconnected global variables where one global variable affected another global variable etc. 

In addition, the `handle` API, while nice in theory, did not support direct event dispatches. As such, regenerating cache (a very common operation), involved first performing a mess of checks and db calls to regenerate the cache, followed by sending a lot of messages (usually within the same thread itself) to finish the regeneration and update the VFS (virtual file system) used by templates on the VM. This was also error-prone as regenerations could be partially completed and then fail due to the underlying thread missing a channel message, hence leaving the guild in a half-regenerated state. 

Worker guild maps were a mess that required a lot of careful manual management to work right with new missed edge cases constantly being found. The planned process pooling was impossible to implement due to the large slew of global variables. 

Lastly, the old system did not support multiple tenant types making Discord's new user-installable apps impossible to implement.
