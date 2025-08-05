use std::sync::Arc;

use khronos_runtime::{primitives::event::{CreateEvent, Event}, require::FilesystemWrapper, rt::{IsolateData, KhronosIsolate}, utils::khronos_value::KhronosValue};
use serenity::all::{GuildId, ParseIdError};
use std::time::Duration;
use crate::{templatingrt::template::Template, worker::{workercachedata::WorkerCacheData, workerstate::WorkerState}};
use super::workervmmanager::{Id, WorkerVmManager, VmData};
use super::limits::MAX_TEMPLATES_RETURN_WAIT_TIME;
use super::vmcontext::TemplateContextProvider;
use crate::events::{AntiraidEvent, KeyResumeEvent};
use crate::dispatch::parse_event;

/// The result from a template execution
pub type TemplateResult = Result<KhronosValue, crate::Error>;

/// A WorkerDispatch manages the dispatching of events to a Luau VM
/// with some utility methods
#[derive(Clone)]
pub struct WorkerDispatch {
    /// VM Manager for the worker
    vm_manager: WorkerVmManager,
    /// Worker State
    state: WorkerState,
    /// Worker Cache Data (needed for dispatching resume keys)
    cache: WorkerCacheData,
}

impl WorkerDispatch {
    /// Creates a new WorkerDispatch with the given WorkerVmManager
    pub fn new(vm_manager: WorkerVmManager, state: WorkerState, cache: WorkerCacheData) -> Self {
        Self { vm_manager, state, cache }
    }

    /// Helper method to dispatch an scoped event to the right templates given a tenant ID and an event
    pub async fn dispatch_scoped_event_to_templates(&self, id: Id, event: CreateEvent, scopes: &[String]) -> Result<Vec<(String, TemplateResult)>, crate::Error> {
        let templates = self.cache.get_templates_with_event_scoped(id, &event, scopes).await;
        self.dispatch_event(id, event, templates).await
    }

    /// Dispatches resume keys to a tenant
    pub async fn dispatch_resume_keys(&self, id: Id) -> Result<(), crate::Error> {
        match id {
            Id::GuildId(guild_id) => {
                self.dispatch_resume_keys_for_guild(guild_id).await
            }
        }
    }

    /// Dispatches resume keys for a guild
    async fn dispatch_resume_keys_for_guild(&self, guild_id: GuildId) -> Result<(), crate::Error> {
        #[derive(sqlx::FromRow)]
        struct KeyResumePartial {
            id: String,
            key: String,
            scopes: Vec<String>,
        }

        let partials: Vec<KeyResumePartial> =
            sqlx::query_as("SELECT id, key, scopes FROM guild_templates_kv WHERE resume = true AND guild_id = $1")
                .bind(guild_id.to_string())
                .fetch_all(&self.state.pool)
                .await?;

        for partial in partials {
            let scopes = partial.scopes.clone();
            log::info!(
                "Dispatching key resume event for key: {} and scopes {:?}",
                partial.key,
                partial.scopes
            );

            let event = AntiraidEvent::KeyResume(KeyResumeEvent {
                id: partial.id,
                key: partial.key,
                scopes: partial.scopes,
            });

            let tevent = parse_event(&event)?;

            if let Err(e) = self.dispatch_scoped_event_to_templates(Id::GuildId(guild_id), tevent, &scopes).await {
                log::error!("Failed to dispatch initiate resume key event for guild {guild_id}: {e}");
            }
        }

        Ok(())
    }

    /// Dispatches resume keys for all tenants
    /// 
    /// Currently only supports guild tenants
    pub async fn dispatch_resume_keys_to_all(&self) -> Result<(), crate::Error> {
        #[derive(sqlx::FromRow)]
        struct KeyResumePartial {
            id: String,
            key: String,
            scopes: Vec<String>,
            guild_id: String
        }

        let partials: Vec<KeyResumePartial> =
            sqlx::query_as("SELECT guild_id, id, key, scopes FROM guild_templates_kv WHERE resume = true")
                .fetch_all(&self.state.pool)
                .await?;

        for partial in partials {
            let guild_id: GuildId = partial.guild_id.parse().map_err(|e: ParseIdError| e.to_string())?;
            let scopes = partial.scopes.clone();
            log::info!(
                "Dispatching key resume event for key: {} and scopes {:?} in guild {}",
                partial.key,
                partial.scopes,
                guild_id
            );

            let event = AntiraidEvent::KeyResume(KeyResumeEvent {
                id: partial.id,
                key: partial.key,
                scopes: partial.scopes,
            });

            let tevent = parse_event(&event)?;

            if let Err(e) = self.dispatch_scoped_event_to_templates(Id::GuildId(guild_id), tevent, &scopes).await {
                log::error!("Failed to dispatch initiate resume key event for guild {guild_id}: {e}");
            }
        }

        Ok(())
    }

    /// Helper method to regenerate the template cache for a guild. This refetches the templates
    /// into cache
    /// 
    /// This is mainly useful during a deferred cache regeneration in which we need to be able to
    /// regenerate the cache+VM 
    pub async fn regenerate_cache(&self, pool: &sqlx::PgPool, id: Id) -> Result<(), crate::Error> {
        self.cache.regenerate_templates_for(pool, id).await?; // Regenerate templates
        self.cache.regenerate_key_expiries_for(pool, id).await?; // Regenerate key expiries too
        self.vm_manager.remove_vm_for(id)?; // Remove the VM to force recreation 
        self.dispatch_resume_keys(id).await?; // Dispatch resume keys after reload

        Ok(())
    }

    /// Dispatches an event to the appropriate VM based on the tenant ID without waiting for a response
    pub async fn dispatch_event(&self, id: Id, event: CreateEvent, templates: Vec<Arc<Template>>) -> Result<Vec<(String, TemplateResult)>, crate::Error> {
        let vm_data = self.vm_manager.get_vm_for(id).await
            .map_err(|e| format!("Failed to get VM for ID {id:?}: {e}"))?;

        self.dispatch_event_to_templates(templates, event, &vm_data, id).await
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
        vm_data.state.serenity_context.http.send_message(
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
            let err = vm_data.state.serenity_context.http.send_message(
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
    async fn dispatch_event_to_template(
        template: &Arc<Template>,
        event: Event,
        vm_data: &VmData,
        id: Id,
    ) -> Result<KhronosValue, crate::Error> {
        if vm_data.runtime_manager.runtime().is_broken() {
            return Err("Lua VM to dispatch to is broken".into());
        }

        // Get or create a subisolate
        let (sub_isolate, created_context) = if let Some(sub_isolate) =
            vm_data.runtime_manager.get_sub_isolate(&template.name)
        {
            (sub_isolate.isolate, sub_isolate.data)
        } else {
            let mut attempts = 0;
            let sub_isolate = loop {
                // It may take a few attempts to create a subisolate successfully
                // due to ongoing Lua VM operations
                match KhronosIsolate::new_subisolate(
                    vm_data.runtime_manager.runtime().clone(),
                    FilesystemWrapper::new(template.content.0.clone()),
                    false // TODO: Allow safeenv optimization one day
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

            log::info!("Created subisolate for template {}", template.name);

            let provider = TemplateContextProvider::new(vm_data.state.clone(), template.clone(), id);

            let created_context = match sub_isolate.create_context(provider) {
                Ok(ctx) => ctx,
                Err(e) => {
                    return Err(format!("Failed to create context for template {}: {}", template.name, e).into());
                }
            };

            let iso_data = IsolateData {
                isolate: sub_isolate.clone(),
                data: created_context.clone(),
            };

            vm_data.runtime_manager.add_sub_isolate(template.name.clone(), iso_data);

            (sub_isolate, created_context)
        };

        let spawn_result = sub_isolate
            .spawn_asset("/init.luau", "/init.luau", created_context, event)
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
    async fn dispatch_event_to_templates(
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

        let event = Event::from_create_event_with_runtime(vm_data.runtime_manager.runtime(), event)
            .map_err(|e| format!("Failed to create event: {}", e))?;

        for template in templates {
            let vm_ref = vm_data.clone();
            let event_ref = event.clone();
            set.spawn_local(async move {
                let name = template.name.clone();
                let result = Self::dispatch_event_to_template(&template, event_ref, &vm_ref, id).await;

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

        // TODO: Support deferred cache regen
        /*let data = vm_data.state.serenity_context.data::<crate::Data>();
        if let Err(e) = regenerate_deferred(&vm_data.state.serenity_context, &data, guild_state.guild_id).await {
            log::error!("Failed to regenerate deferred: {}", e);
        };*/

        Ok(results)
    }
}