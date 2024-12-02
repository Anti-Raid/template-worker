use serenity::all::{
    Context, CreateActionRow, CreateButton, CreateEmbed, CreateMessage, FullEvent, GuildId,
    Interaction,
};
use silverpelt::ar_event::{AntiraidEvent, EventHandlerContext};
use silverpelt::data::Data;
use std::sync::Arc;
use templating::{
    cache::get_all_guild_templates,
    event::{ArcOrNormal, Event},
    GuildTemplate, Template,
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

    let Some(guild_id) = gwevent::core::get_event_guild_id(&event) else {
        return Ok(());
    };

    let event_snake_name = event.snake_case_name();
    if not_audit_loggable_event().contains(&event_snake_name) {
        return Ok(());
    }

    let user_id = gwevent::core::get_event_user_id(&event);

    // Ignore ourselves as well as interaction creates that are reserved
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
        _ => match user_id {
            Some(user_id) => {
                if user_id == serenity_context.cache.current_user().id {
                    return Ok(());
                }
            }
            None => {}
        },
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
            false,
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
                    false,
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
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(&sting)?)),
                    false,
                    None,
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
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(&sting)?)),
                    false,
                    None,
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
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(&sting)?)),
                    false,
                    None,
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
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(&punishment)?.into())),
                    false,
                    None,
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
                    ArcOrNormal::Arc(Arc::new(serde_json::to_value(&punishment)?)),
                    false,
                    None,
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
                    false,
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
        if !filters.contains(&event_name.to_string()) {
            return false;
        }

        return true;
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

    for template in templates.iter().filter(|template| {
        should_dispatch_event(&event.name(), {
            // False positive, unwrap_or_default cannot be used here as it moves the event out of the sink
            if let Some(ref events) = template.events {
                events
            } else {
                &[]
            }
        })
    }) {
        match templating::execute::<Option<()>>(
            guild_id,
            Template::Named(template.name.clone()),
            data.pool.clone(),
            ctx.clone(),
            data.reqwest.clone(),
            event.clone(),
        )
        .await
        {
            Ok(_) => {}
            Err(e) => {
                dispatch_error(ctx, data, &e.to_string(), guild_id, template).await?;
            }
        }
    }
    Ok(())
}

/// Dispatches an error event
async fn dispatch_error(
    ctx: &Context,
    data: &Data,
    error: &str,
    guild_id: GuildId,
    template: &GuildTemplate,
) -> Result<(), silverpelt::Error> {
    let templates = get_all_guild_templates(guild_id, &data.pool).await?;

    if templates.is_empty() {
        return Ok(());
    }

    match template.error_channel {
        Some(c) => {
            let Some(channel) =
                sandwich_driver::channel(&ctx.cache, &ctx.http, &data.reqwest, Some(guild_id), c)
                    .await?
            else {
                return Ok(());
            };

            let Some(guild_channel) = channel.guild() else {
                return Ok(());
            };

            if guild_channel.guild_id != guild_id {
                return Ok(());
            }

            c.send_message(
                &ctx.http,
                CreateMessage::new()
                    .embed(
                        CreateEmbed::new()
                            .title("Error executing template")
                            .field("Error", error, false)
                            .field("Template", template.name.clone(), false),
                    )
                    .components(vec![CreateActionRow::Buttons(
                        vec![
                            CreateButton::new_link(&config::CONFIG.meta.support_server_invite)
                                .label("Support Server"),
                        ]
                        .into(),
                    )]),
            )
            .await?;
        }
        None => {
            // Try firing the error event
            templating::execute::<Option<()>>(
                guild_id,
                Template::Named(template.name.clone()),
                data.pool.clone(),
                ctx.clone(),
                data.reqwest.clone(),
                Event::new(
                    "Error".to_string(),
                    "Error".to_string(),
                    "Error".to_string(),
                    ArcOrNormal::Normal(error.into()),
                    false,
                    None,
                ),
            )
            .await?;
        }
    }

    Ok(())
}
