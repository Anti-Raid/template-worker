use serenity::all::GuildId;
use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::Error;

use super::base_template::BaseTemplateRef;
use super::template_shop_listing::TemplateShopListingRef;

/// Represents a template owned by a guild
/// or one that is attached to a template shop listing
pub struct AttachedGuildTemplate {
    /// The ID of the guild that has the template attached to it
    pub guild_id: GuildId,

    /// Reference to the base template pool
    pub template_pool_ref: BaseTemplateRef,

    /// Reference to the shop listing if
    /// the template is part of a shop listing
    pub shop_listing_ref: Option<TemplateShopListingRef>, // TODO: Replace with actual ShopListingRef type

    /// When the template was attached to the guild
    pub created_at: DateTime<Utc>,

    /// Allowed capabilities for this template
    pub allowed_caps: Vec<String>,

    /// Events associated with this template
    pub events: Vec<String>,
}

impl AttachedGuildTemplate {
    /// Fetches all AttachedGuildTemplates for a given guild
    #[allow(dead_code)]
    pub async fn fetch_all_for_guild(
        pool: &sqlx::PgPool,
        guild_id: GuildId,
    ) -> Result<Vec<Self>, Error> {
        let rows = sqlx::query(r#"
                SELECT 
                    guild_id,
                    template_pool_ref,
                    shop_listing_ref,
                    created_at,
                    allowed_caps,
                    events 
                FROM attached_guild_templates 
                WHERE guild_id = $1"#
            )
            .bind(guild_id.to_string())
            .fetch_all(pool)
            .await?;

        let mut templates = Vec::new();
        for row in rows {
            let db_template = AttachedGuildTemplateDb::try_from(row)?;
            let template = db_template.into_attached_guild_template()?;
            templates.push(template);
        }

        Ok(templates)
    }

    /// Fetches a AttachedGuildTemplate from the database
    /// by template ref id
    #[allow(dead_code)]
    pub async fn fetch_by_template_ref(
        pool: &sqlx::PgPool,
        guild_id: GuildId,
        template_pool_ref: &BaseTemplateRef,
    ) -> Result<Option<Self>, Error> {
        let row = sqlx::query(r#"
                SELECT 
                    guild_id,
                    template_pool_ref,
                    shop_listing_ref,
                    created_at,
                    allowed_caps,
                    events 
                FROM attached_guild_templates 
                WHERE guild_id = $1 AND template_pool_ref = $2"#
            )
            .bind(guild_id.to_string())
            .bind(template_pool_ref.id())
            .fetch_optional(pool)
            .await?;

        if let Some(row) = row {
            let db_template = AttachedGuildTemplateDb::try_from(row)?;
            let template = db_template.into_attached_guild_template()?;
            Ok(Some(template))
        } else {
            Ok(None)
        }
    }

    /// Returns if the guild template comes from a shop listing or not
    pub fn is_from_shop_listing(&self) -> bool {
        self.shop_listing_ref.is_some()
    }
}


/// Simple intermediary struct for DB -> AttachedGuildTemplate conversion
struct AttachedGuildTemplateDb {
    guild_id: String,
    template_pool_ref: Uuid,
    shop_listing_ref: Option<Uuid>,
    created_at: DateTime<Utc>,
    allowed_caps: Vec<String>,
    events: Vec<String>,
}

impl AttachedGuildTemplateDb {
    /// Convert a (internal) AttachedGuildTemplateDb to a AttachedGuildTemplate
    fn into_attached_guild_template(self) -> Result<AttachedGuildTemplate, Error> {
        Ok(AttachedGuildTemplate {
            guild_id: self.guild_id.parse().map_err(|_| "Invalid guild ID")?,
            template_pool_ref: BaseTemplateRef::new(self.template_pool_ref),
            shop_listing_ref: self.shop_listing_ref.map(TemplateShopListingRef::new),
            created_at: self.created_at,
            allowed_caps: self.allowed_caps,
            events: self.events,
        })
    }
} 

impl TryFrom<PgRow> for AttachedGuildTemplateDb {
    type Error = Error;
    fn try_from(row: PgRow) -> Result<Self, Self::Error> {
        Ok(AttachedGuildTemplateDb {
            guild_id: row.try_get("guild_id")?,
            template_pool_ref: row.try_get("template_pool_ref")?,
            shop_listing_ref: row.try_get("shop_listing_ref")?,
            created_at: row.try_get("created_at")?,
            allowed_caps: row.try_get("allowed_caps")?,
            events: row.try_get("events")?,
        })
    }
}