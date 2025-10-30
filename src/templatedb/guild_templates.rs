use serenity::all::{UserId, GuildId};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgRow;
use sqlx::{Executor, Postgres, Row};
use uuid::Uuid;

use crate::Error;

use super::base_template::BaseTemplateRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AttachmentSource {
    ShopListing,
    Created
}

impl AttachmentSource {
    /// Create a new AttachmentSource
    /// 
    /// Returns [`None`] if the source_type is invalid
    pub fn new(source_type: &str) -> Option<Self> {
        match source_type {
            "shop_listing" => Some(AttachmentSource::ShopListing),
            "created" => Some(AttachmentSource::Created),
            _ => None,
        }
    }

    /// Returns if the source is from a shop listing
    #[allow(dead_code)]
    pub fn is_shop_listing(&self) -> bool {
        matches!(self, AttachmentSource::ShopListing)
    }

    /// Returns if the source is from a created template
    #[allow(dead_code)]
    pub fn is_created(&self) -> bool {
        matches!(self, AttachmentSource::Created)
    }
}

/// Represents a 'reference' (which is anyone who is using the template)
pub enum TemplateReference {
    /// A guild that has the template attached
    Guild {
        guild_id: GuildId,
    },
    User {
        user_id: UserId,
    },
}

/// Represents a template owned by a guild
/// or one that is attached to a template shop listing
pub struct AttachedGuildTemplate {
    /// The ID of the guild that has the template attached to it
    pub guild_id: GuildId,

    /// Reference to the base template pool
    pub template_pool_ref: BaseTemplateRef,

    /// The source of how this template was attached
    pub source: AttachmentSource,

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
                    source,
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
                    source,
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

    /// Returns the references to the base template
    pub async fn template_refs<'c, E>(&self, db: E) -> Result<Vec<TemplateReference>, Error> 
        where E: Executor<'c, Database = Postgres>
    {
        let mut refs = Vec::new();
        refs.push(TemplateReference::Guild {
            guild_id: self.guild_id,
        });

        if self.source.is_shop_listing() {
            // Add refs from other guilds that have this template attached
            let rows = sqlx::query(r#"
                SELECT guild_id 
                FROM attached_guild_templates 
                WHERE template_pool_ref = $1"#
            )
            .bind(self.template_pool_ref.id())
            .fetch_all(db)
            .await?;

            for row in rows {
                let guild_id_str: String = row.try_get("guild_id")?;
                let guild_id = guild_id_str.parse().map_err(|_| "Invalid guild ID")?;
                if guild_id == self.guild_id {
                    continue;
                }
                refs.push(TemplateReference::Guild {
                    guild_id,
                });
            }
        }

        Ok(refs)
    }

    /// Updates the guild template's allowed capabilities and events
    /// 
    /// This is allowed for both owned and attached templates
    /// 
    /// Dev note: this is the underlying DB code. Workers should use
    /// a dedicated API within WorkerCacheData or WorkerDB which will
    /// call this and handle/trigger cache regeneration etc as needed.
    #[allow(dead_code)]
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
        let mut tx = pool.begin().await?;

        // Determine if we need to fully delete the template pool from db or not
        let fully_delete = {
            if self.source.is_created() {
                true
            } else {
                let refs = self.template_refs(&mut *tx).await?;
                refs.len() <= 1 // Only this guild
            }
        };

        if !fully_delete {
            // Not owner, just delete the attached template
            // reference as we only support shop listings
            // for shared ownership right now and even
            // if this changes in the future, we will
            // still probably want to just detach here.
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
        } else {
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

        Ok(())
    }
}


/// Simple intermediary struct for DB -> AttachedGuildTemplate conversion
struct AttachedGuildTemplateDb {
    guild_id: String,
    template_pool_ref: Uuid,
    source: String,
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
            source: AttachmentSource::new(&self.source).ok_or("Invalid attachment source")?,
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
            source: row.try_get("source")?,
            created_at: row.try_get("created_at")?,
            allowed_caps: row.try_get("allowed_caps")?,
            events: row.try_get("events")?,
        })
    }
}