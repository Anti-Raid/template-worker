mod atomicinstant;
pub mod cache;
pub mod core;

mod lang_lua;
pub use core::page::Page;
pub use core::templating_core::{
    create_shop_template, parse_shop_template, GuildTemplate, Template, TemplateLanguage,
    TemplatePragma,
};
pub use lang_lua::ctx::TemplateContextRef;
pub use lang_lua::event;
pub use lang_lua::primitives_docs;
pub use lang_lua::samples;
pub use lang_lua::state::LuaKVConstraints;
pub use lang_lua::PLUGINS;
pub use lang_lua::{handle_event, ArLuaThreadInnerState, LuaVmAction, LuaVmResult};

pub const MAX_TEMPLATE_MEMORY_USAGE: usize = 1024 * 1024 * 3; // 3MB maximum memory
pub const MAX_VM_THREAD_STACK_SIZE: usize = 1024 * 1024 * 8; // 8MB maximum memory
pub const MAX_TEMPLATE_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60 * 15); // 15 minutes maximum lifetime
pub const MAX_TEMPLATES_EXECUTION_TIME: std::time::Duration =
    std::time::Duration::from_secs(60 * 5); // 5 minute maximum execution time

type Error = Box<dyn std::error::Error + Send + Sync>; // This is constant and should be copy pasted

async fn get_template(
    guild_id: serenity::all::GuildId,
    template: &str,
    pool: &sqlx::PgPool,
) -> Result<GuildTemplate, Error> {
    if template.starts_with("$shop/") {
        let (shop_tname, shop_tversion) = parse_shop_template(template)?;

        let shop_template = sqlx::query!(
            "SELECT name, description, content, created_at, created_by, last_updated_at, last_updated_by FROM template_shop WHERE name = $1 AND version = $2",
            shop_tname,
            shop_tversion
        )
        .fetch_optional(pool)
        .await?;

        let guild_data = sqlx::query!(
            "SELECT events, error_channel FROM guild_templates WHERE guild_id = $1 AND name = $2",
            guild_id.to_string(),
            template
        )
        .fetch_optional(pool)
        .await?;

        let Some(guild_data) = guild_data else {
            return Err("Guild data not found".into());
        };

        match shop_template {
            Some(shop_template) => Ok(GuildTemplate {
                name: shop_template.name,
                description: Some(shop_template.description),
                shop_name: Some(template.to_string()),
                events: guild_data.events,
                error_channel: match guild_data.error_channel {
                    Some(channel_id) => Some(channel_id.parse()?),
                    None => None,
                },
                content: shop_template.content,
                created_by: shop_template.created_by,
                created_at: shop_template.created_at,
                updated_by: shop_template.last_updated_by,
                updated_at: shop_template.last_updated_at,
            }),
            None => Err("Shop template not found".into()),
        }
    } else {
        let rec = sqlx::query!(
            "SELECT events, content, error_channel, created_at, created_by, last_updated_at, last_updated_by FROM guild_templates WHERE guild_id = $1 AND name = $2",
            guild_id.to_string(),
            template
        )
        .fetch_optional(pool)
        .await?;

        match rec {
            Some(rec) => Ok(GuildTemplate {
                name: template.to_string(),
                description: None,
                shop_name: None,
                events: rec.events,
                error_channel: match rec.error_channel {
                    Some(channel_id) => Some(channel_id.parse()?),
                    None => None,
                },
                content: rec.content,
                created_by: rec.created_by,
                created_at: rec.created_at,
                updated_by: rec.last_updated_by,
                updated_at: rec.last_updated_at,
            }),
            None => Err("Template not found".into()),
        }
    }
}

#[allow(unused_variables)]
pub async fn parse(
    guild_id: serenity::all::GuildId,
    template: Template,
    pool: sqlx::PgPool,
) -> Result<(), Error> {
    let template_content = match template {
        Template::Raw(ref template) => template.clone(),
        Template::Named(ref template) => get_template(guild_id, template, &pool).await?.content,
    };

    let (template_content, pragma) = TemplatePragma::parse(&template_content)?;

    Ok(())
}

/// Executes a template
pub async fn execute(
    guild_id: serenity::all::GuildId,
    template: Template,
    pool: sqlx::PgPool,
    serenity_context: serenity::all::Context,
    reqwest_client: reqwest::Client,
    event: event::Event,
) -> Result<(), Error> {
    let template_content = match template {
        Template::Raw(ref template) => template.clone(),
        Template::Named(ref template) => get_template(guild_id, template, &pool).await?.content,
    };

    let (template_content, pragma) = TemplatePragma::parse(&template_content)?;

    match pragma.lang {
        TemplateLanguage::Lua => {
            lang_lua::render_template(
                event,
                lang_lua::ParseCompileState {
                    serenity_context,
                    reqwest_client,
                    guild_id,
                    template,
                    pragma,
                    template_content: template_content.to_string(),
                    pool,
                },
            )
            .await
        }
    }
}

/// Dispatches an error to a channel
pub async fn dispatch_error(
    ctx: &serenity::all::Context,
    error: &str,
    guild_id: serenity::all::GuildId,
    template: &GuildTemplate,
) -> Result<(), silverpelt::Error> {
    let data = ctx.data::<silverpelt::data::Data>();

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
                serenity::all::CreateMessage::new()
                    .embed(
                        serenity::all::CreateEmbed::new()
                            .title("Error executing template")
                            .field("Error", error, false)
                            .field("Template", template.name.clone(), false),
                    )
                    .components(vec![serenity::all::CreateActionRow::Buttons(
                        vec![serenity::all::CreateButton::new_link(
                            &config::CONFIG.meta.support_server_invite,
                        )
                        .label("Support Server")]
                        .into(),
                    )]),
            )
            .await?;
        }
        None => {
            // Try firing the error event
            execute(
                guild_id,
                Template::Named(template.name.clone()),
                data.pool.clone(),
                ctx.clone(),
                data.reqwest.clone(),
                event::Event::new(
                    "Error".to_string(),
                    "Error".to_string(),
                    "Error".to_string(),
                    event::ArcOrNormal::Normal(error.into()),
                    false,
                    None,
                ),
            )
            .await?;
        }
    }

    Ok(())
}
