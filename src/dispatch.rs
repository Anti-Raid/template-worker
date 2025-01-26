use crate::templatingrt::cache::get_all_guild_templates;
use crate::templatingrt::{dispatch_error, execute, ParseCompileState};
use khronos_runtime::primitives::event::CreateEvent;
use serenity::all::{Context, FullEvent, GuildId, Interaction};
use silverpelt::ar_event::AntiraidEvent;
use silverpelt::data::Data;

#[inline]
const fn not_audit_loggable_event() -> &'static [&'static str] {
    &[
        "CACHE_READY",         // Internal
        "RATELIMIT",           // Internal
        "GUILD_CREATE",        // Internal
        "GUILD_MEMBERS_CHUNK", // Internal
    ]
}

pub async fn discord_event_dispatch(
    event: &FullEvent,
    serenity_context: &Context,
) -> Result<(), silverpelt::Error> {
    let data = serenity_context.data::<Data>();

    let Some(guild_id) = gwevent::core::get_event_guild_id(event) else {
        return Ok(());
    };

    let event_snake_name = event.snake_case_name();
    if not_audit_loggable_event().contains(&event_snake_name) {
        return Ok(());
    }

    let user_id = gwevent::core::get_event_user_id(event);

    let event_data = match event {
        FullEvent::GuildAuditLogEntryCreate { .. } => serde_json::to_value(event)?,
        FullEvent::InteractionCreate { interaction } => {
            match interaction {
                Interaction::Ping(_) => return Ok(()),
                Interaction::Command(i) | Interaction::Autocomplete(i) => {
                    if limits::command_name_limits::RESERVED_COMMAND_NAMES
                        .contains(&i.data.name.as_str())
                    {
                        return Ok(());
                    }

                    let mut value = serde_json::to_value(interaction)?;

                    // Inject in type
                    if let serde_json::Value::Object(ref mut map) = value {
                        let typ: u8 = serenity::all::InteractionType::Command.0;
                        map.insert("type".to_string(), serde_json::Value::Number(typ.into()));
                    }

                    serde_json::json!({
                        "InteractionCreate": {
                            "interaction": value
                        }
                    })
                }
                _ => {
                    let mut value = serde_json::to_value(interaction)?; // Allow Component+Modal interactions to freely passed through

                    // Inject in type
                    if let serde_json::Value::Object(ref mut map) = value {
                        let typ: u8 = interaction.kind().0;
                        map.insert("type".to_string(), serde_json::Value::Number(typ.into()));
                    }

                    serde_json::json!({
                        "InteractionCreate": {
                            "interaction": value
                        }
                    })
                }
            }
        }
        // Ignore ourselves as well as interaction creates that are reserved
        _ => {
            if let Some(user_id) = user_id {
                if user_id == serenity_context.cache.current_user().id {
                    return Ok(());
                }
            }

            serde_json::to_value(event)?
        }
    };

    dispatch(
        serenity_context,
        &data,
        CreateEvent::new(
            "Discord".to_string(),
            event.snake_case_name().to_uppercase(),
            event_data,
            user_id.map(|u| u.to_string()),
        ),
        guild_id,
    )
    .await
}

/// Parses an antiraid event into a template event
pub fn parse_event(event: &AntiraidEvent) -> Result<CreateEvent, silverpelt::Error> {
    Ok(CreateEvent::new(
        "AntiRaid".to_string(),
        event.to_string(),
        event.to_value()?,
        event.author(),
    ))
}

pub async fn dispatch(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
) -> Result<(), silverpelt::Error> {
    let Some(templates) = get_all_guild_templates(guild_id).await else {
        return Ok(());
    };

    for template in templates.iter().filter(|template| {
        template.events.contains(&event.name().to_string())
            || template.events.contains(&event.base_name().to_string())
    }) {
        log::info!("Dispatching event: {} to {}", event.name(), template.name);

        match execute(
            event.clone(),
            ParseCompileState {
                serenity_context: ctx.clone(),
                pool: data.pool.clone(),
                reqwest_client: data.reqwest.clone(),
                guild_id,
            },
            template.clone(),
        )
        .await
        {
            Ok(handle) => {
                let template_name = template.name.clone();
                let serenity_context = ctx.clone();
                tokio::task::spawn(async move {
                    handle
                        .wait_and_log_error(&template_name, guild_id, &serenity_context)
                        .await
                        .map_err(|e| {
                            log::error!("Error while waiting for template: {}", e);
                        })
                });
            }
            Err(e) => {
                dispatch_error(ctx, &e.to_string(), guild_id, template).await?;
            }
        }
    }
    Ok(())
}

/// Dispatches a template event to all templates, waiting for the response and returning it
pub async fn dispatch_and_wait(
    ctx: &Context,
    data: &Data,
    event: CreateEvent,
    guild_id: GuildId,
    wait_timeout: std::time::Duration,
) -> Result<Vec<serde_json::Value>, silverpelt::Error> {
    let Some(templates) = get_all_guild_templates(guild_id).await else {
        return Ok(vec![]);
    };

    let mut local_set = tokio::task::JoinSet::new();
    for template in templates.iter().filter(|template| {
        template.events.contains(&event.name().to_string())
            || template.events.contains(&event.base_name().to_string())
    }) {
        log::info!("Dispatching event: {} to {}", event.name(), template.name);

        match execute(
            event.clone(),
            ParseCompileState {
                serenity_context: ctx.clone(),
                pool: data.pool.clone(),
                reqwest_client: data.reqwest.clone(),
                guild_id,
            },
            template.clone(),
        )
        .await
        {
            Ok(handle) => {
                let template = template.name.clone();
                local_set.spawn(async move {
                    match handle.wait_timeout(wait_timeout).await {
                        Ok(Some(action)) => action
                            .into_response::<serde_json::Value>()
                            .map_err(|e| (e, template)),
                        Ok(None) => Err(("Timed out while waiting for response".into(), template)),
                        Err(e) => Err((e.to_string().into(), template)),
                    }
                });
            }
            Err(e) => return Err(e),
        }
    }

    let mut results = Vec::with_capacity(local_set.len());

    while let Some(result) = local_set.join_next().await {
        let result = result?;
        match result {
            Ok(r) => results.push(r),
            Err((e, template_name)) => {
                local_set.abort_all();

                while (local_set.join_next().await).is_some() {
                    // Drain the rest of the results
                }

                let template = templates.iter().find(|t| t.name == template_name).unwrap();

                if let Err(e) = dispatch_error(ctx, &e.to_string(), guild_id, template).await {
                    log::error!("Error while dispatching error: {}", e);
                };

                return Err(e);
            }
        }
    }

    Ok(results)
}
