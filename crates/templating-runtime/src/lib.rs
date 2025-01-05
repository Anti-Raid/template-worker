mod cache;
mod core;
pub mod doclib; // Temporary?

mod lang_lua;

// Public re-exports
pub use cache::{clear_cache, get_all_guild_templates, get_guild_template};
pub use core::page::Page;
pub use core::templating_core::{
    create_shop_template, parse_shop_template, Template, TemplateLanguage, TemplatePragma,
};
pub use lang_lua::primitives::{document_primitives, CreateEvent, TemplateContextRef}; // Expose CreateEvent to actually execute events
pub use lang_lua::state::LuaKVConstraints;
pub use lang_lua::PLUGINS;
pub use lang_lua::{
    benchmark_vm, dispatch_error, execute, log_error, FireBenchmark, ParseCompileState,
    RenderTemplateHandle,
};

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 3; // 3MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 8; // 8MB maximum memory
pub const MAX_TEMPLATES_EXECUTION_TIME: std::time::Duration =
    std::time::Duration::from_secs(60 * 5); // 5 minute maximum execution time
pub const MAX_TEMPLATES_RETURN_WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(10); // 10 seconds maximum execution time

type Error = Box<dyn std::error::Error + Send + Sync>; // This is constant and should be copy pasted
