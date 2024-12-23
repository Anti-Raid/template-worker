pub use silverpelt::templates::LuaKVConstraints;

pub mod captcha {
    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Captcha {
        pub text: String,
        pub content: Option<String>, // Message content
        pub image: Option<Vec<u8>>,  // Image data
    }
}

pub mod templating_core {
    const MAX_CAPS: usize = 50;
    const MAX_PRAGMA_SIZE: usize = 2048;

    use std::str::FromStr;

    pub use silverpelt::templates::{create_shop_template, parse_shop_template};
    use silverpelt::Error;

    #[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
    pub struct TemplatePragma {
        pub lang: TemplateLanguage,

        #[serde(default)]
        pub allowed_caps: Vec<String>,

        #[serde(flatten)]
        pub extra_info: indexmap::IndexMap<String, serde_json::Value>,
    }

    impl TemplatePragma {
        pub fn parse(template: &str) -> Result<(&str, Self), Error> {
            let (first_line, rest) = match template.find('\n') {
                Some(i) => template.split_at(i),
                None => return Ok((template, Self::default())),
            };

            // Unravel any comments before the @pragma
            let first_line = first_line.trim_start_matches("--").trim();

            if !first_line.contains("@pragma ") {
                return Ok((template, Self::default()));
            }

            // Remove out the @pragma and serde parse it
            let first_line = first_line.replace("@pragma ", "");

            if first_line.len() > MAX_PRAGMA_SIZE {
                return Err("Pragma too large".into());
            }

            let pragma: TemplatePragma = serde_json::from_str(&first_line)?;

            if pragma.allowed_caps.len() > MAX_CAPS {
                return Err("Too many allowed capabilities specified".into());
            }

            Ok((rest, pragma))
        }
    }

    #[derive(Clone, serde::Serialize, serde::Deserialize, Default)]
    pub enum TemplateLanguage {
        #[serde(rename = "lua")]
        #[default]
        Lua,
    }

    impl FromStr for TemplateLanguage {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "lang_lua" => Ok(Self::Lua),
                _ => Err(()),
            }
        }
    }

    impl std::fmt::Display for TemplateLanguage {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Lua => write!(f, "lang_lua"),
            }
        }
    }

    #[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
    pub struct GuildTemplate {
        /// The name of the template
        pub name: String,
        /// The description of the template
        pub description: Option<String>,
        /// The name of the template as it appears on the template shop listing
        pub shop_name: Option<String>,
        /// The events that this template listens to
        pub events: Option<Vec<String>>,
        /// The channel to send errors to
        pub error_channel: Option<serenity::all::ChannelId>,
        /// The content of the template
        pub content: String,
        /// The user who created the template
        pub created_by: String,
        /// The time the template was created
        pub created_at: chrono::DateTime<chrono::Utc>,
        /// The user who last updated the template
        pub updated_by: String,
        /// The time the template was last updated
        pub updated_at: chrono::DateTime<chrono::Utc>,
    }

    impl GuildTemplate {
        pub async fn get(
            guild_id: serenity::all::GuildId,
            template: &str,
            pool: &sqlx::PgPool,
        ) -> Result<Self, Error> {
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
                    Some(shop_template) => Ok(Self {
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
                    Some(rec) => Ok(Self {
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

        /// Converts the guild template to a parsed template struct
        pub fn to_parsed_template(&self) -> Result<ParsedTemplate, Error> {
            let (template_content, pragma) = TemplatePragma::parse(&self.content)?;

            Ok(ParsedTemplate {
                template: Template::Named(self.name.clone()),
                pragma,
                template_content: template_content.to_string(),
            })
        }
    }

    #[derive(Clone, Hash, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    pub enum Template {
        Raw(String),
        Named(String),
    }

    impl Template {
        /// Converts the template to a parsed template struct
        pub async fn to_parsed_template(
            &self,
            guild_id: serenity::all::GuildId,
            pool: &sqlx::PgPool,
        ) -> Result<ParsedTemplate, Error> {
            ParsedTemplate::parse(guild_id, self.clone(), pool).await
        }

        pub async fn into_parsed_template(
            self,
            guild_id: serenity::all::GuildId,
            pool: &sqlx::PgPool,
        ) -> Result<ParsedTemplate, Error> {
            ParsedTemplate::parse(guild_id, self, pool).await
        }
    }

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    /// Represents a template that has been parsed
    pub struct ParsedTemplate {
        pub template: Template,
        pub pragma: TemplatePragma,
        pub template_content: String,
    }

    impl ParsedTemplate {
        #[allow(unused_variables)]
        pub async fn parse(
            guild_id: serenity::all::GuildId,
            template: Template,
            pool: &sqlx::PgPool,
        ) -> Result<Self, Error> {
            let template_content = match template {
                Template::Raw(ref template) => template.clone(),
                Template::Named(ref template) => {
                    GuildTemplate::get(guild_id, template, pool).await?.content
                }
            };

            let (template_content, pragma) = TemplatePragma::parse(&template_content)?;

            Ok(Self {
                template,
                pragma,
                template_content: template_content.to_string(),
            })
        }
    }
}

pub mod page {
    pub const MAX_PAGE_ID_LENGTH: usize = 128;

    pub struct Page {
        pub page_id: String,
        pub title: String,
        pub description: String,
        pub template: crate::Template,
        pub settings: Vec<ar_settings::types::Setting>,
    }
}
