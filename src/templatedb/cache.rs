use std::{collections::HashMap, fmt::Debug, sync::Arc};

use sqlx::Postgres;

use crate::{templatedb::{attached_templates::{AttachedTemplate, TemplateOwner, TemplateSource}, computed_template::ComputedTemplate, template_shop_listing::{ShopListingId, TemplateShopListing}}, worker::workerfilter::WorkerFilter};

/// Stores an in-memory cache of templates and their attachments
#[derive(Debug)]
pub struct TemplateCache {
    attachments: HashMap<TemplateOwner, Vec<ComputedTemplate>>,
}

#[allow(dead_code)]
impl TemplateCache {
    /// Create a new, empty TemplateCache
    pub async fn new<'c>(db: &mut sqlx::Transaction<'c, Postgres>, filter: WorkerFilter) -> Result<Self, crate::Error> {
        let attached_templates = AttachedTemplate::fetch_all(&mut **db).await?;

        let mut attached_map = HashMap::with_capacity(attached_templates.len());
        let mut shop_templates: HashMap<ShopListingId, Arc<TemplateShopListing>> = HashMap::new();

        for at in attached_templates {
            #[allow(deprecated)]
            if !filter.is_allowed(at.owner.to_id()) {
                continue;
            }

            let shop_listing = match at.source {
                TemplateSource::Shop { shop_listing } => {
                    let l = if let Some(listing) = shop_templates.get(&shop_listing) {
                        listing.clone()
                    } else {
                        let listing = TemplateShopListing::fetch_by_id(&mut **db, shop_listing)
                        .await?
                        .ok_or("internal error: shop listing not found for attached template")?;
                        let arc_listing = Arc::new(listing);
                        shop_templates.insert(shop_listing, arc_listing.clone());
                        arc_listing
                    };

                    Some(l)
                }
                _ => None
            };

            attached_map.entry(at.owner)
                .or_insert_with(Vec::new)
                .push(ComputedTemplate::compute(Arc::new(at), shop_listing)?);
        }

        Ok(TemplateCache {
            attachments: attached_map,
        })
    }
}