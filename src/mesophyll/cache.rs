use std::{collections::HashMap, fmt::Debug, sync::Arc};

use serde::{Deserialize, Serialize};
use sqlx::Postgres;

use crate::{templatedb::{attached_templates::{AttachedTemplate, TemplateOwner, AttachedTemplateId}}, worker::workerfilter::WorkerFilter};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheRegenTriggerMode {
    None, // No regeneration triggered
    CausedByTemplateRemoval, // Caused when a template attachment is removed
    CausedByTemplateUpsertExisting, // Caused when an existing template attachment is upserted
    CausedByTemplateUpsertNew, // Caused when a new owner entry is created while upserting
    CausedByFullSync, // Caused by a full sync operation
    ManuallyTriggered, // Caused by a manual trigger (not automatic)
}

#[allow(dead_code)]
impl CacheRegenTriggerMode {
    /// Returns true if the cache regeneration was triggered by an event
    pub fn is_triggered(&self) -> bool {
        match self {
            CacheRegenTriggerMode::None => false,
            _ => true,
        }
    }
}

#[derive(Debug)]
pub struct TemplateOwnerCache {
    templates: HashMap<AttachedTemplateId, Arc<AttachedTemplate>>,
    cache_regen_triggered: CacheRegenTriggerMode,
}

/// Store a filtered view of a TemplateCache
#[derive(Debug)]
pub struct TemplateCacheView {
    entries: HashMap<TemplateOwner, TemplateOwnerCache>,
}

#[allow(dead_code)]
impl TemplateCacheView {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Apply a TemplateCacheUpdate to this view
    /// 
    /// Returns the list of affected TemplateReferences, if any
    pub fn apply_cache_update(&mut self, update: TemplateCacheUpdate) {
        self.apply_cache_update_single(update, 0);
    }

    /// Internal recursive function to apply a TemplateCacheUpdate
    fn apply_cache_update_single(&mut self, update: TemplateCacheUpdate, depth: usize) {
        if depth > 16 {
            log::error!("TemplateCacheView::apply_cache_update_single: exceeded max recursion depth");
            return;
        }

        match update {
            TemplateCacheUpdate::Multi { evt } => {
                for single_update in evt {
                    self.apply_cache_update_single(single_update, depth + 1);
                }
            }
            TemplateCacheUpdate::Flush {} => {
                self.entries.clear();
            }
            TemplateCacheUpdate::FullSyncOwner { owner, templates } => {
                // If no templates, remove the owner entry
                if templates.is_empty() {
                    self.entries.remove(&owner);
                    return;
                }

                let mut template_map = HashMap::with_capacity(templates.len());
                for at in templates {
                    template_map.insert(at.id, at);
                }
                self.entries.insert(owner, TemplateOwnerCache {
                    templates: template_map,
                    cache_regen_triggered: CacheRegenTriggerMode::CausedByFullSync,
                });
            }
            TemplateCacheUpdate::UpsertTemplateAttachment { attachment } => {
                if let Some(attachments) = self.entries.get_mut(&attachment.owner) {
                    attachments.templates.insert(attachment.id, attachment);
                    attachments.cache_regen_triggered = CacheRegenTriggerMode::CausedByTemplateUpsertExisting;
                } else {
                    let owner = attachment.owner;
                    let mut new_map = HashMap::new();
                    new_map.insert(attachment.id, attachment);
                    self.entries.insert(owner, TemplateOwnerCache {
                        templates: new_map,
                        cache_regen_triggered: CacheRegenTriggerMode::CausedByTemplateUpsertNew,
                    });
                }
            }
            TemplateCacheUpdate::RemoveTemplateAttachment { owner, template_ref } => {
                if let Some(attachments) = self.entries.get_mut(&owner) {
                    attachments.templates.remove(&template_ref);
                    attachments.cache_regen_triggered = CacheRegenTriggerMode::CausedByTemplateRemoval;
                }
            }
            TemplateCacheUpdate::RegenerateCache { owner } => {
                if let Some(attachments) = self.entries.get_mut(&owner) {
                    attachments.cache_regen_triggered = CacheRegenTriggerMode::ManuallyTriggered;
                }
            }
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub enum TemplateCacheUpdate {
    /// Performs multiple updates in a single message
    /// 
    /// Used for the initial state sync as well as batch updates
    Multi {
        evt: Vec<TemplateCacheUpdate>,
    },
    /// Flushes the current cache
    Flush {},
    /// Performs a full sync on a specific owner
    FullSyncOwner {
        owner: TemplateOwner,
        templates: Vec<Arc<AttachedTemplate>>,
    },
    /// Add or remove a attached template
    UpsertTemplateAttachment { 
        attachment: Arc<AttachedTemplate>,
    },
    RemoveTemplateAttachment { 
        owner: TemplateOwner,
        template_ref: AttachedTemplateId,
    },
    RegenerateCache {
        owner: TemplateOwner,
    },
}

/// Stores an in-memory cache of templates and their attachments
#[derive(Debug)]
pub struct TemplateCache {
    attachments: HashMap<TemplateOwner, Vec<Arc<AttachedTemplate>>>,
}

#[allow(dead_code)]
impl TemplateCache {
    /// Create a new, empty TemplateCache
    pub async fn new<'c>(db: &mut sqlx::Transaction<'c, Postgres>) -> Result<Self, crate::Error> {
        let attached_templates = AttachedTemplate::fetch_all(&mut **db).await?;

        let mut attached_map = HashMap::with_capacity(attached_templates.len());

        for at in attached_templates {
            attached_map.entry(at.owner)
                .or_insert_with(Vec::new)
                .push(at.into());
        }

        Ok(TemplateCache {
            attachments: attached_map,
        })
    }

    /// Creates a Sync update for a worker using a WorkerFilter
    ///
    /// The resulting sync can be sent to the destination worker to sync all data
    pub fn create_full_sync(&self, filter: &WorkerFilter, flush: bool) -> TemplateCacheUpdate {
        let mut events = Vec::new();
        if flush {
            events.push(TemplateCacheUpdate::Flush {});
        }

        for (owner, attachments) in &self.attachments {
            #[allow(deprecated)]
            if filter.is_allowed(owner.to_id()) {
                events.push(TemplateCacheUpdate::FullSyncOwner {
                    owner: *owner,
                    templates: attachments.clone(),
                });
            }
        }

        TemplateCacheUpdate::Multi { evt: events }
    }

    /// Creates a FullSyncOwner update for a specific owner
    pub fn create_full_sync_owner(&self, owner: TemplateOwner) -> TemplateCacheUpdate {
        let attachments = self.attachments.get(&owner)
            .cloned()
            .unwrap_or_default();

        TemplateCacheUpdate::FullSyncOwner {
            owner,
            templates: attachments,
        }
    } 

    /// Creates an Upsert attachment update
    pub fn create_upsert_attachment(&self, attachment: Arc<AttachedTemplate>) -> TemplateCacheUpdate {
        TemplateCacheUpdate::UpsertTemplateAttachment { attachment }
    }

    /// Creates a Remove attachment update
    pub fn create_remove_attachment(&self, owner: TemplateOwner, template_ref: AttachedTemplateId) -> TemplateCacheUpdate {
        TemplateCacheUpdate::RemoveTemplateAttachment { owner, template_ref }
    }
}