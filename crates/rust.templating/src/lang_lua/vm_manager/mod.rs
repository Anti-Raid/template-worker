mod perthreadpanichook;

#[cfg(feature = "thread_proc")]
mod threadperguild_strategy;
#[cfg(feature = "threadpool_proc")]
mod threadpool_strategy;

#[cfg(feature = "thread_proc")]
pub use threadperguild_strategy::create_lua_vm;
#[cfg(feature = "threadpool_proc")]
pub use threadpool_strategy::create_lua_vm;
