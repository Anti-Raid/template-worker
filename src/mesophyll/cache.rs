use std::{collections::HashMap, fmt::Debug, sync::Arc};

use serde::{Deserialize, Serialize};
use sqlx::Postgres;

use crate::{templatedb::{attached_templates::NormalAttachedTemplate, base_template::{BaseTemplate, BaseTemplateRef, TemplateOwner, TemplateReference}}, worker::workerfilter::WorkerFilter};

/// Store a filtered view of a TemplateCache
#[derive(Debug, Serialize, Deserialize)]
pub struct TemplateCacheView {
    template_refs: HashMap<BaseTemplateRef, Arc<BaseTemplate>>,
    attachments: HashMap<TemplateOwner, Vec<Arc<NormalAttachedTemplate>>>,
}

impl TemplateCacheView {
    /// Apply a TemplateCacheUpdate to this view
    /// 
    /// Returns the list of affected TemplateReferences, if any
    pub fn apply_cache_update(&mut self, update: TemplateCacheUpdate) -> Option<Vec<TemplateReference>> {
        match update {
            TemplateCacheUpdate::AddTemplatePool { template } => {
                self.template_refs.insert(template.id, template);
                None
            }
            TemplateCacheUpdate::UpdateTemplatePool { template, affected_refs } => {
                self.template_refs.insert(template.id, template);
                Some(affected_refs)
            }
            TemplateCacheUpdate::RemoveTemplatePool { template_ref, affected_refs } => {
                self.template_refs.remove(&template_ref);
                Some(affected_refs)
            }
            TemplateCacheUpdate::AddTemplateAttachment { attachment } => {
                self.attachments.entry(attachment.owner)
                    .or_insert_with(Vec::new)
                    .push(attachment);
                None
            }
            TemplateCacheUpdate::UpdateTemplateAttachment { attachment } => {
                if let Some(attachments) = self.attachments.get_mut(&attachment.owner) {
                    if let Some(pos) = attachments.iter().position(|at| at.template_pool_ref == attachment.template_pool_ref) {
                        attachments[pos] = attachment;
                    } else {
                        attachments.push(attachment);
                    }
                } else {
                    self.attachments.insert(attachment.owner, vec![attachment]);
                }
                None
            }
            TemplateCacheUpdate::RemoveTemplateAttachment { owner, template_ref } => {
                if let Some(attachments) = self.attachments.get_mut(&owner) {
                    attachments.retain(|at| at.template_pool_ref != template_ref);
                }
                None
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TemplateCacheUpdate {
    /// Insert a new template into the pool
    AddTemplatePool { 
        template: Arc<BaseTemplate> 
    },
    /// Update an existing template in the pool
    /// 
    /// Also contains a list of affected references which may need a cache regen operation
    UpdateTemplatePool { 
        template: Arc<BaseTemplate>,
        affected_refs: Vec<TemplateReference>,
    },
    RemoveTemplatePool { 
        template_ref: BaseTemplateRef,
        affected_refs: Vec<TemplateReference>,
    },
    AddTemplateAttachment { 
        attachment: Arc<NormalAttachedTemplate> 
    },
    UpdateTemplateAttachment { 
        attachment: Arc<NormalAttachedTemplate> 
    },
    RemoveTemplateAttachment { 
        owner: TemplateOwner,
        template_ref: BaseTemplateRef,
    },
}

/// Stores an in-memory cache of templates and their attachments
#[derive(Debug)]
pub struct TemplateCache {
    template_refs: HashMap<BaseTemplateRef, Arc<BaseTemplate>>,
    attachments: HashMap<TemplateOwner, Vec<Arc<NormalAttachedTemplate>>>,
}

#[allow(dead_code)]
impl TemplateCache {
    /// Create a new, empty TemplateCache
    pub async fn new<'c>(db: &mut sqlx::Transaction<'c, Postgres>) -> Result<Self, crate::Error> {
        let attached_templates = NormalAttachedTemplate::fetch_all(db).await?;

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
        })
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