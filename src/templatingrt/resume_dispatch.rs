use crate::data::Data;
use antiraid_types::ar_event::AntiraidEvent;
use serenity::all::{GuildId, ParseIdError};
use antiraid_types::ar_event::KeyResumeEvent;
use crate::dispatch::{dispatch_scoped, parse_event};

/// Dispatches resume keys for a guild
pub async fn dispatch_resume_keys(
    context: &serenity::all::Context,
    data: &Data,
    guild_id: GuildId,
) -> Result<(), crate::Error> {
    #[derive(sqlx::FromRow)]
    struct KeyResumePartial {
        id: String,
        key: String,
        scopes: Vec<String>,
    }

    let partials: Vec<KeyResumePartial> =
        sqlx::query_as("SELECT id, key, scopes FROM guild_templates_kv WHERE resume = true AND guild_id = $1")
            .bind(guild_id.to_string())
            .fetch_all(&data.pool)
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

        dispatch_scoped(
            context,
            data,
            tevent,
            &scopes,
            guild_id,
        )
        .await?;
    }

    Ok(())
}

/// Dispatches resume keys for all guild
pub async fn dispatch_resume_keys_to_all(
    context: &serenity::all::Context,
    data: &Data,
) -> Result<(), crate::Error> {
    #[derive(sqlx::FromRow)]
    struct KeyResumePartial {
        id: String,
        key: String,
        scopes: Vec<String>,
        guild_id: String
    }

    let partials: Vec<KeyResumePartial> =
        sqlx::query_as("SELECT guild_id, id, key, scopes FROM guild_templates_kv WHERE resume = true")
            .fetch_all(&data.pool)
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

        dispatch_scoped(
            context,
            data,
            tevent,
            &scopes,
            guild_id,
        )
        .await?;
    }

    Ok(())
}