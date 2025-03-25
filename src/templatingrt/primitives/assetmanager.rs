//! Temporary until templating supports multifile scripts in full

use std::{borrow::Cow, sync::Arc};

use khronos_runtime::utils::assets::AssetManager;

use crate::templatingrt::template::Template;

/// An asset manager is responsible for loading read-only assets.
///
/// This can/will be used in AntiRaid (at least) for multifile scripts
#[derive(Clone)]
pub struct TemplateAssetManager {
    /// The template itself
    pub template: Arc<Template>,
}

impl AssetManager for TemplateAssetManager {
    fn get_file(&self, path: &str) -> Result<Cow<'_, str>, khronos_runtime::Error> {
        if path == "init.luau" {
            return Ok(Cow::Borrowed(&self.template.content));
        }

        Err("multifile scripts not supported yet".into())
    }
}
