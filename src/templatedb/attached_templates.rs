use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row};
use uuid::Uuid;

use crate::Error;
use crate::templatedb::base_template::TemplateOwner;

use super::base_template::BaseTemplateRef;

/// Template state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateState {
    Active,
    Paused,
    Suspended,
}

impl FromStr for TemplateState {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(TemplateState::Active),
            "paused" => Ok(TemplateState::Paused),
            "suspended" => Ok(TemplateState::Suspended),
            _ => Err("Invalid template state".into()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

/// Represents a template owned by a guild
/// or one that is attached to a template shop listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachedTemplate {
    /// The owner of the template usage wise
    pub owner: TemplateOwner,

    /// Reference to the base template pool
    pub template_pool_ref: BaseTemplateRef,

    /// The source of how this template was attached
    pub source: AttachmentSource,

    /// When the template was attached to the guild
    pub created_at: DateTime<Utc>,

    /// When the template was last updated
    pub last_updated_at: DateTime<Utc>,

    /// Allowed capabilities for this template
    pub allowed_caps: Vec<String>,

    /// State of the template
    pub state: TemplateState,

    /// Events associated with this template
    pub events: Vec<String>,
}

impl AttachedTemplate {
    /// Fetches all AttachedTemplate's for a given owner
    #[allow(dead_code)]
    pub async fn fetch_all<'c>(
        db: &mut sqlx::Transaction<'c, Postgres>
    ) -> Result<Vec<Self>, Error> {
        let rows = sqlx::query(r#"
                SELECT 
                    owner_type,
                    owner_id,
                    template_pool_ref,
                    source,
                    created_at,
                    last_updated_at,
                    allowed_caps,
                    state,
                    events 
                FROM attached_templates"#
            )
            .fetch_all(&mut (**db))
            .await?;

        let mut templates = Vec::new();
        for row in rows {
            let db_template = AttachedTemplateDb::try_from(row)?;
            let template = db_template.into_attached_guild_template()?;
            templates.push(template);
        }

        Ok(templates)
    }

    /// Fetches all AttachedTemplate's for a given owner
    #[allow(dead_code)]
    pub async fn fetch_all_for_owner(
        pool: &sqlx::PgPool,
        owner: TemplateOwner,
    ) -> Result<Vec<Self>, Error> {
        let rows = sqlx::query(r#"
                SELECT 
                    owner_type,
                    owner_id,
                    template_pool_ref,
                    source,
                    created_at,
                    last_updated_at,
                    allowed_caps,
                    state,
                    events 
                FROM attached_templates 
                WHERE owner_type = $1
                AND owner_id = $2"#
            )
            .bind(owner.owner_type())
            .bind(owner.owner_id())
            .fetch_all(pool)
            .await?;

        let mut templates = Vec::new();
        for row in rows {
            let db_template = AttachedTemplateDb::try_from(row)?;
            let template = db_template.into_attached_guild_template()?;
            templates.push(template);
        }

        Ok(templates)
    }

    /// Fetches a AttachedTemplate from the database
    /// by template ref id
    /// 
    /// NOTE: This method does not check ownership,
    #[allow(dead_code)]
    pub async fn fetch_by_template_ref(
        pool: &sqlx::PgPool,
        template_pool_ref: &BaseTemplateRef,
    ) -> Result<Option<Self>, Error> {
        let row = sqlx::query(r#"
                SELECT 
                    owner_type,
                    owner_id,
                    template_pool_ref,
                    source,
                    created_at,
                    last_updated_at,
                    allowed_caps,
                    state,
                    events 
                FROM attached_templates 
                WHERE template_pool_ref = $1"#
            )
            .bind(template_pool_ref.id())
            .fetch_optional(pool)
            .await?;

        if let Some(row) = row {
            let db_template = AttachedTemplateDb::try_from(row)?;
            let template = db_template.into_attached_guild_template()?;
            Ok(Some(template))
        } else {
            Ok(None)
        }
    }

    /// Garbage collect the template if it has no references
    pub async fn gc<'c>(template_pool_ref: BaseTemplateRef, db: &mut sqlx::Transaction<'c, Postgres>) -> Result<(), Error> {
        let refs = template_pool_ref.template_refs(db).await?;
        if refs.is_empty() {
            // No references, delete the base template
            sqlx::query(
                r#"
                DELETE FROM template_pool
                WHERE id = $1
                "#,
            )
            .bind(template_pool_ref.id())
            .execute(&mut (**db))
            .await?;
        }

        Ok(())
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
            UPDATE attached_templates
            SET allowed_caps = $1, events = $2
            WHERE owner_type = $3 AND owner_id = $4 AND template_pool_ref = $5
            "#,
        )
        .bind(&new_allowed_caps)
        .bind(&new_events)
        .bind(self.owner.owner_type())
        .bind(self.owner.owner_id())
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

        // First delete the attached template
        sqlx::query(
            r#"
            DELETE FROM attached_templates
            WHERE owner_type = $1 AND owner_id = $2 AND template_pool_ref = $2
            "#,
        )
        .bind(self.owner.owner_type())
        .bind(self.owner.owner_id())
        .bind(self.template_pool_ref.id())
        .execute(&mut *tx)
        .await?;

        // Then perform garbage collection to remove from template pool
        // if there are no more references
        Self::gc(self.template_pool_ref, &mut tx).await?;

        tx.commit().await?;

        Ok(())
    }
}


/// Simple intermediary struct for DB -> AttachedTemplate conversion
struct AttachedTemplateDb {
    owner_type: String,
    owner_id: String,
    template_pool_ref: Uuid,
    source: String,
    created_at: DateTime<Utc>,
    last_updated_at: DateTime<Utc>,
    allowed_caps: Vec<String>,
    state: TemplateState,
    events: Vec<String>,
}

impl AttachedTemplateDb {
    /// Convert a (internal) AttachedTemplateDb to a AttachedTemplate
    fn into_attached_guild_template(self) -> Result<AttachedTemplate, Error> {
        Ok(AttachedTemplate {
            owner: TemplateOwner::new(&self.owner_type, &self.owner_id)
                .ok_or("Invalid template owner")?,
            template_pool_ref: BaseTemplateRef::new(self.template_pool_ref),
            source: AttachmentSource::new(&self.source).ok_or("Invalid attachment source")?,
            created_at: self.created_at,
            last_updated_at: self.last_updated_at,
            allowed_caps: self.allowed_caps,
            state: self.state,
            events: self.events,
        })
    }
} 

impl TryFrom<PgRow> for AttachedTemplateDb {
    type Error = Error;
    fn try_from(row: PgRow) -> Result<Self, Self::Error> {
        Ok(AttachedTemplateDb {
            owner_type: row.try_get("owner_type")?,
            owner_id: row.try_get("owner_id")?,
            template_pool_ref: row.try_get("template_pool_ref")?,
            source: row.try_get("source")?,
            created_at: row.try_get("created_at")?,
            last_updated_at: row.try_get("last_updated_at")?,
            allowed_caps: row.try_get("allowed_caps")?,
            state: {
                let state_str: String = row.try_get("state")?;
                TemplateState::from_str(&state_str)?
            },
            events: row.try_get("events")?,
        })
    }
}