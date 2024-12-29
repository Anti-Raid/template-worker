mod atomicinstant;
pub mod cache;
pub mod core;
mod serenitystore;

mod lang_lua;
pub use core::page::Page;
pub use core::templating_core::{
    create_shop_template, parse_shop_template, GuildTemplate, ParsedTemplate, Template,
    TemplateLanguage, TemplatePragma,
};
pub use lang_lua::ctx::TemplateContextRef;
pub use lang_lua::event;
pub use lang_lua::primitives_docs;
pub use lang_lua::state::LuaKVConstraints;
pub use lang_lua::PLUGINS;
pub use lang_lua::{
    dispatch_error, execute, handle_event, ArLuaThreadInnerState, LuaVmAction, LuaVmResult,
    ParseCompileState, RenderTemplateHandle,
};
pub use serenitystore::{
    setup_shard_messenger, shard_count, shard_ids, shard_messenger_for_guild,
    update_shard_messengers,
};

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 3; // 3MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 8; // 8MB maximum memory
pub const MAX_TEMPLATE_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60 * 15); // 15 minutes maximum lifetime
pub const MAX_TEMPLATES_EXECUTION_TIME: std::time::Duration =
    std::time::Duration::from_secs(60 * 5); // 5 minute maximum execution time
pub const MAX_TEMPLATES_RETURN_WAIT_TIME: std::time::Duration = std::time::Duration::from_secs(10); // 10 seconds maximum execution time

type Error = Box<dyn std::error::Error + Send + Sync>; // This is constant and should be copy pasted
