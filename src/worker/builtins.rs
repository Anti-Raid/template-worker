use std::{collections::HashMap, sync::{Arc, LazyLock}};

use khronos_runtime::{core::typesext::Vfs, mluau_require::{self, create_memory_vfs_from_embedded}};
use rust_embed::Embed;

/// Builtins
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/bot"]
#[prefix = ""]
pub struct Builtins;

/// To make uploads not need to upload all of ``templating-types`` and keep them up to date:
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/bot/templating-types"]
#[prefix = "templating-types/"]
pub struct TemplatingTypes;

pub static BUILTINS: LazyLock<Arc<mluau_require::Vfs>> = LazyLock::new(|| {
    Arc::new(create_memory_vfs_from_embedded::<Builtins>())
});
pub static TEMPLATING_TYPES: LazyLock<Arc<mluau_require::Vfs>> = LazyLock::new(|| {
    Arc::new(create_memory_vfs_from_embedded::<TemplatingTypes>())
});

pub static EXPOSED_VFS: LazyLock<HashMap<String, Vfs>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("Builtins".to_string(), Vfs::new(BUILTINS.clone(), false));
    map.insert("TemplatingTypes".to_string(), Vfs::new(TEMPLATING_TYPES.clone(), false));
    map
});