use serenity::all::{Context, FullEvent, GuildId, Interaction};
use silverpelt::ar_event::{AntiraidEvent, EventHandlerContext};
use silverpelt::data::Data;
use std::sync::Arc;
use templating::{
    cache::get_all_guild_templates,
    event::{ArcOrNormal, Event},
};

use crate::temporary_punishments;

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

    match event {
        FullEvent::GuildAuditLogEntryCreate { .. } => {}
        FullEvent::InteractionCreate { interaction } => {
            match interaction {
                Interaction::Ping(_) => return Ok(()),
                Interaction::Command(i) | Interaction::Autocomplete(i) => {
                    if limits::command_name_limits::RESERVED_COMMAND_NAMES
                        .contains(&i.data.name.as_str())
                    {
                        return Ok(());
                    }
                }
                _ => {} // Allow Component+Modal interactions to freely passed through
            }
        }
        // Ignore ourselves as well as interaction creates that are reserved
        _ => {
            if let Some(user_id) = user_id {
                if user_id == serenity_context.cache.current_user().id {
                    return Ok(());
                }
            }
        }
    }

    // Convert to titlecase by capitalizing the first letter of each word
    let event_titlename = event
        .snake_case_name()
        .split('_')
        .map(|s| {
            let mut c = s.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().chain(c).collect(),
            }
        })
        .collect::<Vec<String>>()
        .join(" ");

    dispatch(
        serenity_context,
        &data,
        Event::new(
            event_titlename,
            "Discord".to_string(),
            event.snake_case_name().to_uppercase(),
            ArcOrNormal::Arc(Arc::new(serde_json::to_value(event)?)),
            user_id.map(|u| u.to_string()),
        ),
        guild_id,
    )
    .await
}

pub async fn event_listener(ectx: EventHandlerContext) -> Result<(), silverpelt::Error> {
    let ctx = &ectx.serenity_context;

    match ectx.event {
        AntiraidEvent::Custom(ref event) => {
            dispatch(
                ctx,
                &ectx.data,
                Event::new(
                    event.event_titlename.clone(),
                    "Custom".to_string(),
                    event.event_name.clone(),
                    ArcOrNormal::Arc(Arc::new(event.event_data.clone())),
                    None,
                ),
                ectx.guild_id,
            )
            .await
        }
        AntiraidEvent::StingCreate(ref sting) => {
            dispatch(
                ctx,
                &ectx.data,
                Event::new(
                    "(Anti Raid) Sting Created".to_string(),
                    "StingCreate".to_string(),
                    "StingCreate".to_string(),
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(sting)?)),
                    Some(sting.creator.to_string()),
                ),
                ectx.guild_id,
            )
            .await?;

            Ok(())
        }
        AntiraidEvent::StingUpdate(ref sting) => {
            dispatch(
                ctx,
                &ectx.data,
                Event::new(
                    "(Anti Raid) Sting Updated".to_string(),
                    "StingUpdate".to_string(),
                    "StingUpdate".to_string(),
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(sting)?)),
                    Some(sting.creator.to_string()),
                ),
                ectx.guild_id,
            )
            .await?;

            Ok(())
        }
        AntiraidEvent::StingExpire(ref sting) => {
            dispatch(
                ctx,
                &ectx.data,
                Event::new(
                    "(Anti Raid) Sting Expired".to_string(),
                    "StingExpire".to_string(),
                    "StingExpire".to_string(),
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(sting)?)),
                    Some(sting.creator.to_string()),
                ),
                ectx.guild_id,
            )
            .await?;

            Ok(())
        }
        AntiraidEvent::StingDelete(ref sting) => {
            dispatch(
                ctx,
                &ectx.data,
                Event::new(
                    "(Anti Raid) Sting Deleted".to_string(),
                    "StingDelete".to_string(),
                    "StingDelete".to_string(),
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(sting)?)),
                    Some(sting.creator.to_string()),
                ),
                ectx.guild_id,
            )
            .await?;

            Ok(())
        }
        AntiraidEvent::PunishmentCreate(ref punishment) => {
            dispatch(
                ctx,
                &ectx.data,
                Event::new(
                    "(Anti Raid) Punishment Created".to_string(),
                    "PunishmentCreate".to_string(),
                    "PunishmentCreate".to_string(),
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(punishment)?)),
                    Some(punishment.creator.to_string()),
                ),
                ectx.guild_id,
            )
            .await?;

            Ok(())
        }
        AntiraidEvent::PunishmentExpire(ref punishment) => {
            dispatch(
                ctx,
                &ectx.data,
                Event::new(
                    "(Anti Raid) Punishment Expired".to_string(),
                    "PunishmentExpire".to_string(),
                    "PunishmentExpire".to_string(),
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(punishment)?)),
                    Some(punishment.creator.to_string()),
                ),
                ectx.guild_id,
            )
            .await?;

            temporary_punishments::handle_expired_punishment(&ectx.data, ctx, punishment).await?;

            Ok(())
        }
        AntiraidEvent::OnStartup(ref modified) => {
            dispatch(
                ctx,
                &ectx.data,
                Event::new(
                    "(Anti Raid) On Startup".to_string(),
                    "OnStartup".to_string(),
                    "OnStartup".to_string(),
                    ArcOrNormal::Arc(Arc::new(serde_json::json!({
                            "targets": modified
                        }
                    ))),
                    None,
                ),
                ectx.guild_id,
            )
            .await
        }
    }
}

/// Check if an event matches a list of filters
///
/// Rules:
/// - If filter is empty, return true unless a special case applies
/// - If filter matches the event_name, return true unless a special case applies
///
/// Special cases:
/// - If event_name is MESSAGE, then it must be an exact match to be dispatched AND must have a custom template declared for it. This is to avoid spam
fn should_dispatch_event(event_name: &str, filters: &[String]) -> bool {
    if event_name == "MESSAGE" || event_name == "AR/CheckCommand" || event_name == "AR/OnStartup" {
        // Message should only be fired if the template explicitly wants the event
        return filters.contains(&event_name.to_string());
    }

    // If empty, always return Ok
    if filters.is_empty() {
        return true;
    }

    filters.contains(&event_name.to_string())
}

pub async fn dispatch(
    ctx: &Context,
    data: &Data,
    event: Event,
    guild_id: GuildId,
) -> Result<(), silverpelt::Error> {
    let templates = get_all_guild_templates(guild_id, &data.pool).await?;

    if templates.is_empty() {
        return Ok(());
    }

    println!("Dispatching event: {}", event.name());

    for template in templates.iter().filter(|template| {
        should_dispatch_event(event.name(), {
            // False positive, unwrap_or_default cannot be used here as it moves the event out of the sink
            #[allow(clippy::manual_unwrap_or_default)]
            if let Some(ref events) = template.events {
                events
            } else {
                &[]
            }
        })
    }) {
        log::info!("Dispatching event: {} to {}", event.name(), template.name);

        match templating::execute(
            event.clone(),
            templating::ParseCompileState {
                serenity_context: ctx.clone(),
                pool: data.pool.clone(),
                reqwest_client: data.reqwest.clone(),
                guild_id,
            },
            template.to_parsed_template()?,
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                templating::dispatch_error(ctx, &e.to_string(), guild_id, template).await?;
            }
        }
    }
    Ok(())
}
