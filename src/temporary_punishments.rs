use antiraid_types::punishments::{Punishment, PunishmentTarget};
use serenity::http;

use crate::templatingrt::primitives::sandwich_config;

/// Temporary Punishments event listener
pub async fn handle_expired_punishment(
    data: &silverpelt::data::Data,
    serenity_context: &serenity::all::Context,
    punishment: &Punishment,
) -> Result<(), silverpelt::Error> {
    let target_user_id = match punishment.target {
        PunishmentTarget::User(user_id) => user_id,
        _ => return Ok(()),
    };

    let bot_id = serenity_context.cache.current_user().id;

    let guild = sandwich_driver::guild(
        &serenity_context.cache,
        &serenity_context.http,
        &data.reqwest,
        punishment.guild_id,
        &sandwich_config(),
    )
    .await?;

    let current_user = match sandwich_driver::member_in_guild(
        &serenity_context.cache,
        &serenity_context.http,
        &data.reqwest,
        punishment.guild_id,
        bot_id,
        &sandwich_config(),
    )
    .await?
    {
        Some(user) => user,
        None => {
            return Err("Bot is not in the guild".into());
        }
    };

    let permissions = botox::serenity_backports::member_permissions(&guild, &current_user);

    // Bot doesn't have permissions to unban
    if !permissions.ban_members() {
        return Err("Bot doesn't have permissions to unban".into());
    }

    let reason = format!(
        "Revert expired ban with reason={}, duration={:#?}",
        punishment.reason, punishment.duration
    );

    match punishment.punishment.as_str() {
        "ban" => {
            if let Err(e) = punishment
                .guild_id
                .unban(&serenity_context.http, target_user_id, Some(&reason))
                .await
            {
                match e {
                    serenity::Error::Http(http_err) => {
                        if [http::StatusCode::NOT_FOUND, http::StatusCode::FORBIDDEN].contains(
                            &http_err
                                .status_code()
                                .unwrap_or(http::StatusCode::NOT_ACCEPTABLE),
                        ) {
                            return Err(format!("Failed to unban user: {}", http_err).into());
                        }
                    }
                    _ => return Err(Box::new(e)),
                }
            }
        }
        "timeout" => {
            punishment
                .guild_id
                .edit_member(
                    &serenity_context.http,
                    target_user_id,
                    serenity::all::EditMember::new()
                        .enable_communication()
                        .audit_log_reason(&reason),
                )
                .await?;
        }
        "removeallroles" => {
            punishment
                .guild_id
                .edit_member(
                    &serenity_context.http,
                    target_user_id,
                    serenity::all::EditMember::new()
                        .roles(Vec::new())
                        .audit_log_reason(&reason),
                )
                .await?;
        }
        _ => {
            return Ok(());
        }
    }

    Ok(())
}
