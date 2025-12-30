use std::{collections::HashMap, sync::{Arc, LazyLock}};

use khronos_runtime::core::typesext::Vfs;
use rust_embed::Embed;

/// Builtins
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/builtins"]
#[prefix = ""]
pub struct Builtins;

/// To make uploads not need to upload all of ``templating-types`` and keep them up to date:
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/builtins/templating-types"]
#[prefix = "templating-types/"]
pub struct TemplatingTypes;

/// Builtins patches
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/luau/builtins_patches"]
#[prefix = ""]
pub struct BuiltinsPatches;

pub static EXPOSED_VFS: LazyLock<HashMap<String, Vfs>> = LazyLock::new(|| {
    let mut map = HashMap::new();
    map.insert("Builtins".to_string(), Vfs {
        vfs: Arc::new(vfs::EmbeddedFS::<Builtins>::new()),
    });
    map.insert("TemplatingTypes".to_string(), Vfs {
        vfs: Arc::new(vfs::EmbeddedFS::<TemplatingTypes>::new()),
    });
    map.insert("BuiltinsPatches".to_string(), Vfs {
        vfs: Arc::new(vfs::EmbeddedFS::<BuiltinsPatches>::new()),
    });
    map
});