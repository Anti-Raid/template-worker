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

    /// Updates the guild template's allowed capabilities and events
    /// 
    /// This is allowed for both owned and attached templates
    /// 
    /// Dev note: this is the underlying DB code. Workers should use
    /// a dedicated API within WorkerCacheData or WorkerDB which will
    /// call this and handle/trigger cache regeneration etc as needed.
    pub async fn update_caps_and_events(
        &mut self,
        pool: &sqlx::PgPool,
        new_allowed_caps: Vec<String>,
        new_events: Vec<String>,
    ) -> Result<(), Error> {
        sqlx::query(
            r#"
            UPDATE attached_guild_templates
            SET allowed_caps = $1, events = $2
            WHERE guild_id = $3 AND template_pool_ref = $4
            "#,
        )
        .bind(&new_allowed_caps)
        .bind(&new_events)
        .bind(self.guild_id.to_string())
        .bind(self.template_pool_ref.id())
        .execute(pool)
        .await?;

        self.allowed_caps = new_allowed_caps;
        self.events = new_events;

        Ok(())
    }

    /// Deletes the guild template from the database
    ///
    /// This is allowed for both owned and attached templates.
    /// 
    /// If the template is owned (not from a shop listing), then the
    /// pool reference will also be deleted via a transaction.
    /// 
    /// Dev note: this is the underlying DB code. Workers should use
    /// a dedicated API within WorkerCacheData or WorkerDB which will
    /// call this and handle/trigger cache regeneration etc as needed.
    pub async fn delete(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<(), Error> {
        if self.is_from_shop_listing() {
            sqlx::query(
                r#"
                DELETE FROM attached_guild_templates
                WHERE guild_id = $1 AND template_pool_ref = $2
                "#,
            )
            .bind(self.guild_id.to_string())
            .bind(self.template_pool_ref.id())
            .execute(pool)
            .await?;
        } else {
            let mut tx = pool.begin().await?;

            // Verify ownership as an additional step before delete
            // 
            // Only delete the underlying BaseTemplate if the guild owns it
            let Some(owner) = self.template_pool_ref.fetch_owner(&mut *tx).await? else {
                return Err("BaseTemplate not found when attempting to delete owned AttachedGuildTemplate".into());
            };

            sqlx::query(
                r#"
                DELETE FROM attached_guild_templates
                WHERE guild_id = $1 AND template_pool_ref = $2
                "#,
            )
            .bind(self.guild_id.to_string())
            .bind(self.template_pool_ref.id())
            .execute(&mut *tx)
            .await?;

            if owner.guild_owns(self.guild_id) {
                sqlx::query(
                    r#"
                    DELETE FROM template_pool
                    WHERE id = $1
                    "#,
                )
                .bind(self.template_pool_ref.id())
                .execute(&mut *tx)
                .await?;
            }

            tx.commit().await?;
        }

        Ok(())
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