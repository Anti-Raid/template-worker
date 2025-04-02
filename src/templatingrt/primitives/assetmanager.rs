//! Temporary until templating supports multifile scripts in full

use std::{cell::RefCell, collections::HashMap, sync::Arc};

use khronos_runtime::utils::assets::AssetManager;

use crate::templatingrt::template::Template;
use include_dir::include_dir;
use std::sync::LazyLock;

/// To make uploads not need to upload all of ``templating-types``
pub static TEMPLATING_TYPES: std::sync::LazyLock<HashMap<String, Arc<String>>> =
    LazyLock::new(|| {
        let file_contents = include_dir!("$CARGO_MANIFEST_DIR/../../infra/templating-types");

        let mut contents = HashMap::new();

        fn extract_all_paths(map: &mut HashMap<String, Arc<String>>, dir: &include_dir::Dir) {
            for entry in dir.entries() {
                if let Some(dir) = entry.as_dir() {
                    extract_all_paths(map, dir);
                } else {
                    let path = entry.path().to_str().unwrap();
                    let file = entry.as_file().unwrap();
                    let content = String::from_utf8_lossy(file.contents()).to_string();
                    map.insert(path.to_string(), Arc::new(content));
                }
            }
        }

        extract_all_paths(&mut contents, &file_contents);

        contents
    });

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

        log::debug!("Loading asset: {}", path);
        if path.starts_with("templating-types/")
            && template
                .allowed_caps
                .contains(&"assetmanager:use_bundled_templating_types".to_string())
        {
            log::debug!("Loading templating-types from bundle: {}", path);
            if let Some(content) =
                TEMPLATING_TYPES.get(path.trim_start_matches("templating-types/"))
            {
                return Ok(content.clone());
            }
        }

        if let Some(content) = template.content.get(path) {
            println!("{}", content);
            return Ok(content.clone());
        }

        Err(format!("module '{}' not found", path).into())
    }
}
