pub mod captcha {
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Captcha {
        pub text: String,
        pub content: Option<String>, // Message content
        pub image: Option<Vec<u8>>,  // Image data
    }
}

pub mod templating_core {
    use std::str::FromStr;

    pub use silverpelt::templates::{create_shop_template, parse_shop_template};
    use silverpelt::Error;

    #[derive(Clone, serde::Serialize, serde::Deserialize, Default, Debug)]
    pub enum TemplateLanguage {
        #[serde(rename = "luau")]
        #[default]
        Luau,
    }

    impl FromStr for TemplateLanguage {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "luau" => Ok(Self::Luau),
                _ => Err(()),
            }
        }
    }

    impl std::fmt::Display for TemplateLanguage {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Luau => write!(f, "luau"),
            }
        }
    }

    #[derive(serde::Deserialize, serde::Serialize, Clone, Debug, Default)]
    pub struct Template {
        /// The guild id the template is in
        pub guild_id: serenity::all::GuildId,
        /// The name of the template
        pub name: String,
        /// The description of the template
        pub description: Option<String>,
        /// The name of the template as it appears on the template shop listing
        pub shop_name: Option<String>,
        /// The owner of the template on the template shop
        pub shop_owner: Option<serenity::all::GuildId>,
        /// The events that this template listens to
        pub events: Vec<String>,
        /// The channel to send errors to
        pub error_channel: Option<serenity::all::ChannelId>,
        /// The content of the template
        pub content: String,
        /// The language of the template
        pub lang: TemplateLanguage,
        /// The allowed capabilities the template has access to
        pub allowed_caps: Vec<String>,
        /// The user who created the template
        pub created_by: String,
        /// The time the template was created
        pub created_at: chrono::DateTime<chrono::Utc>,
        /// The user who last updated the template
        pub updated_by: String,
        /// The time the template was last updated
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }

    impl Template {
        pub async fn guild(
            guild_id: serenity::all::GuildId,
            template: &str,
            pool: &sqlx::PgPool,
        ) -> Result<Self, Error> {
            if template.starts_with("$shop/") {
                let (shop_tname, shop_tversion) = parse_shop_template(template)?;

                let (
                    st_owner_guild,
                    st_name,
                    st_description,
                    st_content,
                    st_created_at,
                    st_created_by,
                    st_last_updated_at,
                    st_last_updated_by,
                ) = if shop_tversion == "latest" {
                    let rec = sqlx::query!(
                        "SELECT owner_guild, name, description, content, created_at, created_by, last_updated_at, last_updated_by FROM template_shop WHERE name = $1 ORDER BY version DESC LIMIT 1",
                        shop_tname
                    )
                    .fetch_optional(pool)
                    .await?;

                    let rec = rec.ok_or("Shop template not found")?;

                    (
                        rec.owner_guild,
                        rec.name,
                        rec.description,
                        rec.content,
                        rec.created_at,
                        rec.created_by,
                        rec.last_updated_at,
                        rec.last_updated_by,
                    )
                } else {
                    let rec = sqlx::query!(
                        "SELECT owner_guild, name, description, content, created_at, created_by, last_updated_at, last_updated_by FROM template_shop WHERE name = $1 AND version = $2",
                        shop_tname,
                        shop_tversion
                    )
                    .fetch_optional(pool)
                    .await?;

                    let rec = rec.ok_or("Shop template not found")?;

                    (
                        rec.owner_guild,
                        rec.name,
                        rec.description,
                        rec.content,
                        rec.created_at,
                        rec.created_by,
                        rec.last_updated_at,
                        rec.last_updated_by,
                    )
                };

                let guild_data = sqlx::query!(
                    "SELECT language, allowed_caps, events, error_channel FROM guild_templates WHERE guild_id = $1 AND name = $2",
                    guild_id.to_string(),
                    template
                )
                .fetch_optional(pool)
                .await?;

                let Some(guild_data) = guild_data else {
                    return Err("Guild data not found".into());
                };

                Ok(Self {
                    guild_id,
                    name: st_name,
                    description: Some(st_description),
                    shop_name: Some(template.to_string()),
                    shop_owner: Some(st_owner_guild.parse()?),
                    events: guild_data.events,
                    error_channel: match guild_data.error_channel {
                        Some(channel_id) => Some(channel_id.parse()?),
                        None => None,
                    },
                    lang: TemplateLanguage::from_str(&guild_data.language)
                        .map_err(|_| "Invalid language")?,
                    allowed_caps: guild_data.allowed_caps,
                    content: st_content,
                    created_by: st_created_by,
                    created_at: st_created_at,
                    updated_by: st_last_updated_by,
                    updated_at: st_last_updated_at,
                })
            } else {
                let rec = sqlx::query!(
                    "SELECT events, language, allowed_caps, content, error_channel, created_at, created_by, last_updated_at, last_updated_by FROM guild_templates WHERE guild_id = $1 AND name = $2",
                    guild_id.to_string(),
                    template
                )
                .fetch_optional(pool)
                .await?;

                let Some(rec) = rec else {
                    return Err("Template not found".into());
                };

                Ok(Self {
                    guild_id,
                    name: template.to_string(),
                    description: None,
                    shop_name: None,
                    shop_owner: None,
                    events: rec.events,
                    error_channel: match rec.error_channel {
                        Some(channel_id) => Some(channel_id.parse()?),
                        None => None,
                    },
                    content: rec.content,
                    lang: TemplateLanguage::from_str(&rec.language)
                        .map_err(|_| "Invalid language")?,
                    allowed_caps: rec.allowed_caps,
                    created_by: rec.created_by,
                    created_at: rec.created_at,
                    updated_by: rec.last_updated_by,
                    updated_at: rec.last_updated_at,
                })
            }
        }
    }
}

pub mod page {
    use std::sync::Arc;

    pub const MAX_PAGE_ID_LENGTH: usize = 128;

    pub struct Page {
        pub page_id: String,
        pub title: String,
        pub description: String,
        pub template: Arc<crate::Template>,
        pub settings: Vec<ar_settings::types::Setting>,
    }
}
