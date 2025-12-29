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
