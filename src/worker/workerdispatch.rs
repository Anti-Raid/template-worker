use std::sync::Arc;

use khronos_runtime::{primitives::event::{ContextEvent, CreateEvent}, require::FilesystemWrapper, rt::{KhronosIsolate, isolate::CodeSource}, traits::context::TFlags, utils::khronos_value::KhronosValue};
use std::time::Duration;
use super::template::Template;
use crate::worker::{keyexpirychannel::KeyExpiryChannel, workercachedata::{DeferredCacheRegenerationMode, WorkerCacheData}, workerfilter::WorkerFilter};
use super::workervmmanager::{Id, WorkerVmManager, VmData};
use super::limits::MAX_TEMPLATES_RETURN_WAIT_TIME;
use super::vmcontext::TemplateContextProvider;
use crate::events::{AntiraidEvent, KeyResumeEvent};
use crate::dispatch::parse_event;
use super::workerdb::WorkerDB;

/// The result from a template execution
pub type TemplateResult = Result<KhronosValue, crate::Error>;
pub type DispatchTemplateResult = Result<Vec<(String, TemplateResult)>, crate::Error>;

/// A WorkerDispatch manages the dispatching of events to a Luau VM
/// with some utility methods
#[derive(Clone)]
pub struct WorkerDispatch {
    /// VM Manager for the worker
    vm_manager: WorkerVmManager,
    /// Worker Cache Data (needed for cache regen handling)
    cache: WorkerCacheData,
    /// Worker Database
    db: WorkerDB,
    /// Worker filter
    filter: WorkerFilter,
    /// Key expiry channel
    key_expiry_chan: KeyExpiryChannel,
}

impl WorkerDispatch {
    /// Creates a new WorkerDispatch with the given WorkerVmManager
    pub fn new(vm_manager: WorkerVmManager, cache: WorkerCacheData, db: WorkerDB, key_expiry_chan: KeyExpiryChannel, filter: WorkerFilter) -> Self {
        let dispatch = Self { vm_manager, cache, db, key_expiry_chan, filter };

        // Fire resume keys on creation
        let self_ref = dispatch.clone();
        tokio::task::spawn_local(async move {
            if let Err(e) = self_ref.dispatch_resume_keys().await {
                log::error!("Failed to dispatch resume keys on WorkerDispatch creation: {}", e);
            }
        });

        dispatch
    }

    /// Helper method to dispatch an scoped event to the right templates given a tenant ID and an event
    pub async fn dispatch_event_to_templates(&self, id: Id, event: CreateEvent) -> Result<Vec<(String, TemplateResult)>, crate::Error> {
        let templates = self.cache.get_templates_with_event(id, &event);
        self.dispatch_event(id, event, templates).await
    }

    /// Helper method to dispatch an scoped event to the right templates given a tenant ID and an event
    pub async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: &[String]) -> Result<Vec<(String, TemplateResult)>, crate::Error> {
        let templates = self.cache.get_templates_with_event_scoped(id, &event, scopes);
        self.dispatch_event(id, event, templates).await
    }

    /// Dispatches resume keys for all tenants
    pub async fn dispatch_resume_keys(&self) -> Result<(), crate::Error> {
        let resumes_map = self.db.get_resume_keys().await?;
        for (id, resumes) in resumes_map {
            if !self.filter.is_allowed(id) {
                continue;
            }
            
            for resume in resumes {
                log::info!(
                    "Dispatching key resume event for key: {} and scopes {:?} in ID {id:?}",
                    resume.key,
                    resume.scopes
                );

                let event = AntiraidEvent::KeyResume(KeyResumeEvent {
                    id: resume.id,
                    key: resume.key,
                    scopes: resume.scopes.clone(),
                });

                let tevent = parse_event(&event)?;

                let self_ref = self.clone();
                tokio::task::spawn_local(async move {
                    if let Err(e) = self_ref.dispatch_scoped_event_to_templates(id, tevent, &resume.scopes).await {
                        log::error!("Failed to dispatch initiate resume key event for ID {id:?}: {e}");
                    }
                });
            }
        }
        Ok(())
    }

    /// Dispatches resume keys to a tenant
    pub async fn dispatch_resume_keys_for(&self, id: Id) -> Result<(), crate::Error> {
        let resumes = self.db.get_resume_keys_for(id).await?;
        for resume in resumes {
            log::info!(
                "Dispatching key resume event for key: {} and scopes {:?}",
                resume.key,
                resume.scopes
            );

            let event = AntiraidEvent::KeyResume(KeyResumeEvent {
                id: resume.id,
                key: resume.key,
                scopes: resume.scopes.clone(),
            });

            let tevent = parse_event(&event)?;

            let self_ref = self.clone();
            tokio::task::spawn_local(async move {
                if let Err(e) = self_ref.dispatch_scoped_event_to_templates(id, tevent, &resume.scopes).await {
                    log::error!("Failed to dispatch initiate resume key event for ID {id:?}: {e}");
                }
            });
        }

        Ok(())
    }

    // Perform a deferred cache regeneration for a tenant
    pub async fn regenerate_deferred_cache_for(&self, id: Id, mode: DeferredCacheRegenerationMode) -> Result<(), crate::Error> {
        match mode {
            DeferredCacheRegenerationMode::FlushSelf {} => {
                log::info!("Performing deferred cache regeneration for ID {id:?}");
                self.regenerate_cache(id).await?;
            },
            DeferredCacheRegenerationMode::FlushOthers { others } => {
                log::info!("Performing deferred cache regeneration for ID {id:?} and others: {:?}", others);

                for id in others {
                    log::info!("Performing deferred cache regeneration for other tenant ID {id:?}");
                    self.regenerate_cache(id).await?;
                }
            },
        }
        Ok(())
    }

    /// Helper method to regenerate the template cache for a guild. This refetches the templates
    /// into cache and reloads any existing VM for the guild.
    /// 
    /// This is mainly useful during a deferred cache regeneration in which we need to be able to
    /// regenerate the cache+VM 
    pub async fn regenerate_cache(&self, id: Id) -> Result<(), crate::Error> {
        self.cache.repopulate_templates_for(id).await?; // Regenerate templates
        self.vm_manager.remove_vm_for(id)?; // Remove the VM to force recreation 
        self.dispatch_resume_keys_for(id).await?; // Dispatch resume keys after reload

        Ok(())
    }

    /// Dispatches an event to the appropriate VM based on the tenant ID without waiting for a response
    pub async fn dispatch_event(&self, id: Id, event: CreateEvent, templates: Vec<Arc<Template>>) -> DispatchTemplateResult {
        if templates.is_empty() {
            return Ok(Vec::new()); // Fast return if no templates are found. We don't need to even do anything special
        }
       
        let vm_data = self.vm_manager.get_vm_for(id).await
            .map_err(|e| format!("Failed to get VM for ID {id:?}: {e}"))?;

        self.dispatch_event_to_templates_impl(templates, event, &vm_data, id).await
    }

    /// Returns an Discord error message for a template error
    fn error_message(
        template: &Template,
        error: String,
    ) -> serde_json::Value {
        serde_json::json!({
            "embeds": [
                {
                    "title": "Error executing template",
                    "description": error,
                    "fields": [
                        {
                            "name": "Template",
                            "value": template.name.clone(),
                            "inline": false
                        }
                    ],
                }
            ],
            "components": [
                {
                    "type": 1,
                    "components": [
                        {
                            "type": 2,
                            "style": 5,
                            "label": "Support Server",
                            "url": crate::CONFIG.meta.support_server_invite.to_string(),
                        },
                    ]
                }
            ],
        })
    }

    /// Helper method to log a template error to the main server
    async fn log_error_to_main_server(
        vm_data: &VmData,
        template: &Template,
        error: String,
    ) -> Result<(), crate::Error> {
        // Send to main server
        vm_data.state.serenity_http.send_message(
            crate::CONFIG.meta.default_error_channel.widen(),
            Vec::with_capacity(0),
            &Self::error_message(template, error),
        )
        .await?;

        Ok(())
    }

    async fn log_error(
        vm_data: &VmData,
        template: &Template,
        error: String,
    ) -> Result<(), crate::Error> {
        let error = format!("```lua\n{}```", error.replace('`', "\\`"));

        if let Some(error_channel) = template.error_channel {
            let err = vm_data.state.serenity_http.send_message(
                error_channel.widen(),
                Vec::with_capacity(0),
                &Self::error_message(template, error),
            )
            .await;

            // Check for a 404
            if let Err(e) = err {
                match e {
                    serenity::Error::Http(e) => {
                        if let Some(s) = e.status_code() {
                            if s == reqwest::StatusCode::NOT_FOUND {
                                // Remove the error channel
                                match sqlx::query(
                                    "UPDATE templates SET error_channel = NULL WHERE name =$1 AND guild_id = $2",
                                )
                                .bind(&template.name)
                                .bind(template.guild_id.to_string())
                                .execute(&vm_data.state.pool)
                                .await {
                                    Ok(_) => {
                                        // TODO: Add the cache stuff back in once worker cache API is done being reimplemented
                                        // Refresh cache without regenerating
                                        /*get_all_guild_templates_from_db(
                                            template.guild_id,
                                            &guild_state.pool,
                                        )
                                        .await?;*/
                                    },
                                    Err(e) => {
                                        log::error!("Failed to remove error channel for template {}: {}", template.name, e);
                                    }
                                };
                            }
                        }
                    }
                    _ => {}
                }
            }
        } else {
            // If no error channel is set, log to the main server
            Self::log_error_to_main_server(vm_data, template, error)
                .await?;
        }

        Ok(())
    }

    /// Helper method to dispatch an event to a single template
    async fn dispatch_event_to_template_impl(
        template: &Arc<Template>,
        event: ContextEvent,
        vm_data: &VmData,
        cache: WorkerCacheData,
        key_expiry_chan: KeyExpiryChannel,
        id: Id,
    ) -> Result<KhronosValue, crate::Error> {
        if vm_data.runtime_manager.runtime().is_broken() {
            return Err("Lua VM to dispatch to is broken".into());
        }

        let provider = TemplateContextProvider::new(
            vm_data.state.clone(), 
            template.clone(), 
            cache, 
            id,
            vm_data.kv_constraints,
            vm_data.ratelimits.clone(),
            key_expiry_chan
        );

        // Get or create a subisolate
        let (sub_isolate, created_context) = if let Some(sub_isolate) =
            vm_data.runtime_manager.get_sub_isolate(&template.name)
        {
            let created_context = sub_isolate.create_context(provider, event)
                .map_err(|e| format!("Failed to create context for template {}: {}", template.name, e))?;

            (sub_isolate, created_context)
        } else {
            let mut attempts = 0;
            let sub_isolate = loop {
                // It may take a few attempts to create a subisolate successfully
                // due to ongoing Lua VM operations
                match KhronosIsolate::new_subisolate(
                    vm_data.runtime_manager.runtime().clone(),
                    FilesystemWrapper::new(template.content.0.clone()),
                    TFlags::empty() // TODO: Allow safeenv optimization one day
                ) {
                    Ok(isolate) => {
                        break isolate;
                    }
                    Err(e) => {
                        log::error!("WorkerDispatch: Failed to create subisolate: {e}. This is an internal bug that should not happen");
                        attempts += 1;
                        if attempts >= 20 {
                            return Err(format!("Failed to create subisolate: {e}. This is an internal bug that should not happen").into());
                        }

                        // Wait a bit before retrying
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        // Check if the runtime is broken
                        if vm_data.runtime_manager.runtime().is_broken() {
                            return Err("Lua VM to dispatch to is broken".into());
                        }
                    }
                }
            };

            log::debug!("Created subisolate for template {}", template.name);

            vm_data.runtime_manager.add_sub_isolate(template.name.clone(), sub_isolate.clone());

            let created_context = match sub_isolate.create_context(provider, event) {
                Ok(ctx) => ctx,
                Err(e) => {
                    return Err(format!("Failed to create context for template {}: {}", template.name, e).into());
                }
            };

            (sub_isolate, created_context)
        };

        let spawn_result = sub_isolate
            .spawn_asset("/init.luau", CodeSource::AssetPath("/init.luau"), created_context)
            .await
            .map_err(|e| e.to_string())?;

        let value = match spawn_result.into_khronos_value(&sub_isolate) {
            Ok(v) => v,
            Err(e) => {
                return Err(format!("Failed to convert result to JSON: {}", e).into())
            }
        };

        Ok(value)
    }

    /// Dispatches an event to templates and returns the results
    async fn dispatch_event_to_templates_impl(
        &self,
        templates: Vec<Arc<Template>>,
        event: CreateEvent,
        vm_data: &VmData,
        id: Id,
    ) -> Result<Vec<(String, TemplateResult)>, crate::Error> {        
        if vm_data.runtime_manager.runtime().is_broken() {
            return Err("Lua VM to dispatch to is broken".into());
        }

        let num_templates = templates.len();
        log::debug!("Dispatching event to {} templates", num_templates);

        let mut set = tokio::task::JoinSet::new();

        let event = event.into_context();

        for template in templates {
            let vm_ref = vm_data.clone();
            let cache_ref = self.cache.clone();
            let event_ref = event.clone();
            let key_expiry_chan = self.key_expiry_chan.clone();
            set.spawn_local(async move {
                let name = template.name.clone();
                let result = Self::dispatch_event_to_template_impl(&template, event_ref, &vm_ref, cache_ref, key_expiry_chan, id).await;

                if let Err(ref e) = result {
                    // Log the error
                    if let Err(e) = Self::log_error(&vm_ref, &template, e.to_string()).await {
                        log::error!("Failed to log error for template {}: {}", template.name, e);
                    }
                }

                (name, result)
            });
        }

        let mut results = Vec::with_capacity(num_templates);
        while let Ok(Some(result)) =
            tokio::time::timeout(MAX_TEMPLATES_RETURN_WAIT_TIME, set.join_next()).await
        {
            match result {
                Ok((name, result)) => {
                    results.push((name, result));
                }
                Err(e) => {
                    log::error!("Failed to dispatch event to template: {}", e);
                }
            }
        }

        if let Some(mode) = self.cache.take_deferred_cache_regeneration(&id) {
            log::info!("Detected deferred cache regeneration for ID {id:?}, performing now");
            if let Err(e) = self.regenerate_deferred_cache_for(id, mode).await {
                log::error!("Failed to perform deferred cache regeneration for ID {id:?}: {e}");
            }
        }

        Ok(results)
    }
}