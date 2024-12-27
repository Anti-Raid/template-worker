use silverpelt::{ar_event::AntiraidEvent, data::Data};

use crate::dispatch::{dispatch, parse_event};

/// Dispatches OnStartup events for all guilds with templates
pub async fn on_startup(ctx: serenity::all::Context) -> Result<(), silverpelt::Error> {
    let data = ctx.data::<Data>();
    // For every GuildId with templates, fire a OnStartup event
    let guilds = sqlx::query!("SELECT guild_id FROM guild_templates")
        .fetch_all(&data.pool)
        .await?;

    for guild in guilds {
        let Ok(guild_id) = guild.guild_id.parse::<serenity::all::GuildId>() else {
            continue;
        };

        let data = data.clone();
        let ctx = ctx.clone();

        tokio::task::spawn(async move {
            let event = AntiraidEvent::OnStartup(vec![]);
            let event = match parse_event(&event) {
                Ok(event) => event,
                Err(e) => {
                    log::error!("Failed to parse OnStartup event: {}", e);
                    return;
                }
            };

            match dispatch(&ctx, &data, event, guild_id).await {
                Ok(_) => {}
                Err(e) => {
                    log::error!("Failed to dispatch OnStartup event: {}", e);
                }
            };
        });
    }

    Ok(())
}
