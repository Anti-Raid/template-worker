use rust_embed::Embed;
use std::sync::{Arc, LazyLock};

use super::template::{DefaultableOverlayFS, Template};

/// Builtins
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/../../builtins"]
#[prefix = ""]
pub struct Builtins;

/// To make uploads not need to upload all of ``templating-types`` and keep them up to date:
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/../../builtins/templating-types"]
#[prefix = "templating-types/"]
pub struct TemplatingTypes;

/// Builtins patches
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/../../builtins_patches"]
#[prefix = ""]
pub struct BuiltinsPatches;

// Replace this with the new builtins template once ready to deploy
pub const BUILTINS_NAME: &str = "$builtins";
pub static BUILTINS: LazyLock<Arc<Template>> = LazyLock::new(|| {
    let templ = Template {
        content: DefaultableOverlayFS(vfs::OverlayFS::new(&vec![
            vfs::EmbeddedFS::<BuiltinsPatches>::new().into(),
            vfs::EmbeddedFS::<Builtins>::new().into(),
            vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
        ])),
        name: BUILTINS_NAME.to_string(),
        events: vec![
            "INTERACTION_CREATE".to_string(),
            "KeyExpiry[builtins.remindme]".to_string(),
            "GetSettings".to_string(),
            "ExecuteSetting[guildmembers]".to_string(),
            "ExecuteSetting[guildroles]".to_string(),
            "ExecuteSetting[scripts]".to_string(),
        ],
        allowed_caps: vec!["*".to_string()],
        ..Default::default()
    };

    Arc::new(templ)
});
pub const USE_BUILTINS: bool = true;
