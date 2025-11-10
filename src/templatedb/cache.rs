use std::{collections::HashMap, fmt::Debug, sync::Arc};

use serde::{Deserialize, Serialize};
use sqlx::{Executor, Postgres};

use crate::{templatedb::{attached_templates::AttachedTemplate, base_template::{BaseTemplate, BaseTemplateRef, TemplateOwner}}, worker::{workerfilter::WorkerFilter, workervmmanager::Id}};

struct Hook(Option<Arc<dyn Fn(&TemplateCache) + Send + Sync>>);

impl Debug for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hook").finish()
    }
}

/// Stores an in-memory cache of templates and their attachments
#[derive(Debug)]
pub struct TemplateCache {
    template_refs: HashMap<BaseTemplateRef, Arc<BaseTemplate>>,
    attachments: HashMap<TemplateOwner, Vec<Arc<AttachedTemplate>>>,

    // Hooks
    on_update: Hook,
}

/// Store a filtered view of a TemplateCache
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateCacheView {
    template_refs: HashMap<BaseTemplateRef, Arc<BaseTemplate>>,
    attachments: HashMap<TemplateOwner, Vec<Arc<AttachedTemplate>>>,
}

impl TemplateCache {
    /// Create a new, empty TemplateCache
    pub async fn new<'c>(db: &mut sqlx::Transaction<'c, Postgres>) -> Result<Self, crate::Error> {
        let attached_templates = AttachedTemplate::fetch_all(db).await?;

        let mut attached_map = HashMap::with_capacity(attached_templates.len());
        let mut template_refs = HashMap::new();

        for at in attached_templates {
            if !template_refs.contains_key(&at.template_pool_ref) {
                if let Some(base_template) = at.template_pool_ref.fetch_from_db(&mut **db).await? {
                    template_refs.insert(at.template_pool_ref, base_template.into());
                }
            }

            attached_map.entry(at.owner)
                .or_insert_with(Vec::new)
                .push(at.into());
        }

        Ok(TemplateCache {
            template_refs,
            attachments: attached_map,
            on_update: Hook(None)
        })
    }

    /// Sets a hook to be called whenever the cache is updated
    pub fn set_on_update_hook<F>(&mut self, hook: F)
    where
        F: Fn(&TemplateCache) + Send + Sync + 'static,
    {
        self.on_update = Hook(Some(Arc::new(hook)));
    }

    /// 'Distil'/'Filter' a TemplateCache by a WorkerFilter
    ///
    /// Returns a TemplateCacheView containing only the templates and attachments
    /// that the filter allows for the given Id.
    pub fn distil(&self, filter: &WorkerFilter) -> TemplateCacheView {
        let mut filtered_template_refs = HashMap::new();
        let mut filtered_attachments = HashMap::new();

        for (owner, attachments) in &self.attachments {
            if filter.is_allowed(owner.to_id()) {
                filtered_attachments.insert(*owner, attachments.clone());

                for at in attachments.iter() {
                    if let Some(template) = self.template_refs.get(&at.template_pool_ref) {
                        filtered_template_refs.insert(at.template_pool_ref, Arc::clone(template));
                    }
                }
            }
        }

        TemplateCacheView {
            template_refs: filtered_template_refs,
            attachments: filtered_attachments,
        }
    }
}