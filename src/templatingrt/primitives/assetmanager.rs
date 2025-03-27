//! Temporary until templating supports multifile scripts in full

use std::{cell::RefCell, sync::Arc};

use khronos_runtime::utils::assets::AssetManager;

use crate::templatingrt::template::Template;

/// An asset manager is responsible for loading read-only assets.
///
/// This can/will be used in AntiRaid (at least) for multifile scripts
#[derive(Clone)]
pub struct TemplateAssetManager {
    template: RefCell<Arc<Template>>,
}

impl TemplateAssetManager {
    /// Creates a new `TemplateAssetManager` with the given template.
    ///
    /// # Arguments
    ///
    /// * `template` - An `Arc` that holds the template for the asset manager.
    pub fn new(template: Arc<Template>) -> Self {
        Self {
            template: RefCell::new(template),
        }
    }

    /// Sets the template for the template asset manager.
    pub fn set_template(&self, template: Arc<Template>) {
        *self.template.borrow_mut() = template;
    }
}

impl AssetManager for TemplateAssetManager {
    fn get_file(&self, path: &str) -> Result<impl AsRef<String>, khronos_runtime::Error> {
        let template = self.template.borrow();

        if let Some(content) = template.content.get(path) {
            return Ok(content.clone());
        }

        Err(format!("module '{}' not found", path).into())
    }
}
