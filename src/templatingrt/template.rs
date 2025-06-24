use std::str::FromStr;

use super::cache::{TEST_BASE, TEST_BASE_NAME, USE_TEST_BASE};
use khronos_runtime::primitives::event::CreateEvent;
use rust_embed::Embed;
use silverpelt::templates::parse_shop_template;
use silverpelt::Error;

/// To make uploads not need to upload all of ``templating-types`` and keep them up to date:
#[derive(Embed, Debug)]
#[folder = "$CARGO_MANIFEST_DIR/../../infra/templating-types"]
#[prefix = "templating-types/"]
struct TemplatingTypes;

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

#[derive(Clone, Debug)]
/// The constructed filesystem for the template
pub enum ConstructedFS {
    Memory(vfs::MemoryFS),
    Overlay(vfs::OverlayFS),
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
    #[serde(skip)]
    pub content: vfs::MemoryFS,
    /// The constructed filesystem
    #[serde(skip)]
    pub ready_fs: Option<ConstructedFS>,
    /// The language of the template
    pub lang: TemplateLanguage,
    /// The allowed capabilities the template has access to
    pub allowed_caps: Vec<String>,
    /// The user who created the template
    pub created_by: serenity::all::UserId,
    /// The time the template was created
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// The user who last updated the template
    pub updated_by: serenity::all::UserId,
    /// The time the template was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Internal representation of a template in postgres
#[derive(sqlx::FromRow)]
struct TemplateData {
    name: String,
    content: serde_json::Value,
    language: String,
    allowed_caps: Vec<String>,
    events: Vec<String>,
    error_channel: Option<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    created_by: String,
    last_updated_at: chrono::DateTime<chrono::Utc>,
    last_updated_by: String,
}

/// Internal representation of a template in the shop in postgres
#[derive(sqlx::FromRow)]
struct TemplateShopData {
    // owner_guild, name, description, content, created_at, created_by, last_updated_at, last_updated_by
    owner_guild: String,
    name: String,
    description: String,
    content: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    created_by: String,
    last_updated_at: chrono::DateTime<chrono::Utc>,
    last_updated_by: String,
}

impl Template {
    /// Returns true if the event should be dispatched to the template
    pub fn should_dispatch(&self, event: &CreateEvent) -> bool {
        self.events.contains(&event.name().to_string())
            || self.events.contains(&event.base_name().to_string())
    }

    /// Returns true if a scoped event should be dispatched to the template
    pub fn should_dispatch_scoped(&self, event: &CreateEvent, scopes: &[String]) -> bool {
        for scope in scopes {
            if self
                .events
                .contains(&format!("{}[{}]", event.base_name(), scope))
            {
                return true;
            }
        }

        self.should_dispatch(event)
    }

    /// Returns all templates for a guild
    pub async fn guild(
        guild_id: serenity::all::GuildId,
        pool: &sqlx::PgPool,
    ) -> Result<Vec<Self>, Error> {
        let templates: Vec<TemplateData> = sqlx::query_as(
            "SELECT name, content, language, allowed_caps, events, error_channel, created_at, created_by, last_updated_at, last_updated_by FROM guild_templates WHERE guild_id = $1 AND paused = false",
        )
        .bind(guild_id.to_string())
        .fetch_all(pool)
        .await?;

        let mut result = Vec::new();

        for template in templates {
            if template.name.starts_with("$shop/") {
                let (shop_tname, shop_tversion) = parse_shop_template(&template.name)?;

                let shop_data = if shop_tversion == "latest" {
                    let rec: Option<TemplateShopData> = sqlx::query_as(
                        "SELECT owner_guild, name, description, content, created_at, created_by, last_updated_at, last_updated_by FROM template_shop WHERE name = $1 ORDER BY version DESC LIMIT 1",
                    )
                    .bind(shop_tname)
                    .fetch_optional(pool)
                    .await?;

                    let Some(rec) = rec else {
                        continue;
                    };

                    rec
                } else {
                    let rec: Option<TemplateShopData> = sqlx::query_as(
                        "SELECT owner_guild, name, description, content, created_at, created_by, last_updated_at, last_updated_by FROM template_shop WHERE name = $1 AND version = $2",
                    )
                    .bind(shop_tname)
                    .bind(shop_tversion)
                    .fetch_optional(pool)
                    .await?;

                    let Some(rec) = rec else {
                        continue;
                    };

                    rec
                };

                result.push(Self {
                    guild_id,
                    name: template.name,
                    description: Some(shop_data.description),
                    shop_name: Some(shop_data.name),
                    shop_owner: Some(shop_data.owner_guild.parse()?),
                    events: template.events,
                    error_channel: match template.error_channel {
                        Some(channel_id) => Some(channel_id.parse()?),
                        None => None,
                    },
                    lang: TemplateLanguage::from_str(&template.language)
                        .map_err(|_| "Invalid language")?,
                    allowed_caps: template.allowed_caps,
                    content: {
                        let content: std::collections::HashMap<String, String> =
                            serde_json::from_value(shop_data.content)?;

                        khronos_runtime::utils::memoryvfs::create_memory_vfs_from_map(content)
                            .map_err(|e| {
                                Error::from(format!("Failed to create vfs from map: {e}"))
                            })?
                    },
                    ready_fs: None,
                    created_by: shop_data.created_by.parse()?,
                    created_at: shop_data.created_at,
                    updated_by: shop_data.last_updated_by.parse()?,
                    updated_at: shop_data.last_updated_at,
                });
            } else {
                let content = if USE_TEST_BASE && template.name == TEST_BASE_NAME {
                    TEST_BASE.content.clone()
                } else {
                    let content: std::collections::HashMap<String, String> =
                        serde_json::from_value(template.content)?;

                    khronos_runtime::utils::memoryvfs::create_memory_vfs_from_map(content)
                        .map_err(|e| Error::from(format!("Failed to create vfs from map: {e}")))?
                };

                result.push(Self {
                    guild_id,
                    name: template.name.to_string(),
                    description: None,
                    shop_name: None,
                    shop_owner: None,
                    events: template.events,
                    error_channel: match template.error_channel {
                        Some(channel_id) => Some(channel_id.parse()?),
                        None => None,
                    },
                    content,
                    ready_fs: None,
                    lang: TemplateLanguage::from_str(&template.language)
                        .map_err(|_| "Invalid language")?,
                    allowed_caps: template.allowed_caps,
                    created_by: template.created_by.parse()?,
                    created_at: template.created_at,
                    updated_by: template.last_updated_by.parse()?,
                    updated_at: template.last_updated_at,
                });
            }
        }

        Ok(result)
    }

    pub fn prepare_ready_fs(&mut self) {
        let prepped_fs = if self
            .allowed_caps
            .contains(&"assetmanager:use_bundled_templating_types".to_string())
        {
            ConstructedFS::Overlay(vfs::OverlayFS::new(&vec![
                self.content.clone().into(),
                vfs::EmbeddedFS::<TemplatingTypes>::new().into(),
            ]))
        } else {
            ConstructedFS::Memory(self.content.clone())
        };

        log::trace!(
            "Prepared ready fs for template {}: {:?}",
            self.name,
            prepped_fs
        );
        self.ready_fs = Some(prepped_fs);
    }
}
