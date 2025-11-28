use std::sync::Arc;

use khronos_runtime::utils::memoryvfs::create_memory_vfs_from_map;

use crate::templatedb::{attached_templates::{AttachedTemplate, TemplateLanguage, TemplateSource}, builtins::{Builtins, BuiltinsPatches}, template_shop_listing::TemplateShopListing};

use super::builtins::TemplatingTypes;

#[derive(Clone, Debug)]
/// The constructed filesystem for the template
pub struct DefaultableOverlayFS(pub vfs::OverlayFS);

impl std::ops::Deref for DefaultableOverlayFS {
    type Target = vfs::OverlayFS;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Default for DefaultableOverlayFS {
    fn default() -> Self {
        Self(vfs::OverlayFS::new(&vec![
            vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
        ]))
    }
}

#[derive(Clone, Debug)]
/// A computed template with its attached template and the constructed filesystem
pub struct ComputedTemplate {
    pub attached_template: Arc<AttachedTemplate>,
    pub vfs: DefaultableOverlayFS,
    pub lang: TemplateLanguage
}

impl ComputedTemplate {
    /// Create a new ComputedTemplate
    pub fn new(attached_template: Arc<AttachedTemplate>, vfs: DefaultableOverlayFS, lang: TemplateLanguage) -> Self {
        Self {
            attached_template,
            vfs,
            lang,
        }
    }

    /// Create the ComputedTemplate from an AttachedTemplate and a (optional) shop_listing_ref
    /// 
    /// Returns an error if AttachedTemplate is a shop template and no shop_listing_ref is provided
    pub fn compute(attached_template: Arc<AttachedTemplate>, shop_listing: Option<Arc<TemplateShopListing>>) -> Result<Self, crate::Error> {
        let (vfs, lang) = match &attached_template.source {
            TemplateSource::Builtins => {
                (DefaultableOverlayFS(vfs::OverlayFS::new(&vec![
                    vfs::EmbeddedFS::<BuiltinsPatches>::new().into(),
                    vfs::EmbeddedFS::<Builtins>::new().into(),
                    vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
                ])), TemplateLanguage::Luau)
            }
            TemplateSource::Custom { language, content, .. } => {
                let mem_fs =
                    create_memory_vfs_from_map(&content)
                        .map_err(|e| {
                            format!("Failed to create vfs from map: {e}")
                        })?;
                
                (DefaultableOverlayFS(vfs::OverlayFS::new(&vec![
                    mem_fs.into(),
                    vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
                ])), *language)
            }
            TemplateSource::Shop { shop_listing: shop_listing_id } => {
                let Some(listing) = shop_listing else {
                    return Err("internal error: shop listing reference not provided for shop template".into());
                };

                if listing.id != *shop_listing_id {
                    return Err("internal error: provided shop listing does not match attached template's shop listing".into());
                }

                let mem_fs =
                    create_memory_vfs_from_map(&listing.content)
                        .map_err(|e| {
                            format!("Failed to create vfs from map: {e}")
                        })?;

                (DefaultableOverlayFS(vfs::OverlayFS::new(&vec![
                    mem_fs.into(),
                    vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
                ])), listing.language)
            }
        };

        Ok(Self::new(
            attached_template,
            vfs,
            lang,
        ))
    }
}