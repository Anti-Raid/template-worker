use std::collections::HashMap;
use std::fmt::Display;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, UserId};
use sqlx::postgres::PgRow;
use sqlx::{Postgres, Row};
use uuid::Uuid;

use crate::templatedb::template_shop_listing::ShopListingId;
use crate::Error;
use crate::worker::workervmmanager::Id;

#[derive(Clone, serde::Serialize, serde::Deserialize, Default, Debug)]
pub enum TemplateLanguage {
    #[serde(rename = "luau")]
    #[default]
    Luau,
}

impl FromStr for TemplateLanguage {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "luau" => Ok(Self::Luau),
            _ => Err("Invalid template language".into()),
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

/// Template state
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateState {
    #[serde(rename = "active")]
    Active,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "suspended")]
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

impl Display for TemplateState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_str = match self {
            TemplateState::Active => "active",
            TemplateState::Paused => "paused",
            TemplateState::Suspended => "suspended",
        };
        write!(f, "{}", state_str)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TemplateSource {
    Builtins,
    Shop {
        shop_listing: ShopListingId,
    },
    Custom {
        name: String,
        language: TemplateLanguage,
        content: HashMap<String, String>,
    },
}

#[allow(dead_code)]
impl TemplateSource {
    /// Given a PgRow reference, extract the TemplateSource
    fn from_row(row: &PgRow) -> Result<Self, Error> {
        let source_type: String = row.try_get("source")?;
        match source_type.as_str() {
            "builtins" => Ok(TemplateSource::Builtins),
            "shop" => {
                let shop_listing = row.try_get("shop_listing")?;
                Ok(TemplateSource::Shop {
                    shop_listing: ShopListingId::new(shop_listing),
                })
            }
            "custom" => {
                let name = row.try_get("name")?;
                let language: String = row.try_get("language")?;
                let content: serde_json::Value = row.try_get("content")?;

                let content_map: HashMap<String, String> = serde_json::from_value(content)
                    .map_err(|e| format!("Failed to deserialize custom template content: {}", e))?;

                Ok(TemplateSource::Custom {
                    name,
                    language: TemplateLanguage::from_str(&language)
                        .map_err(|_| "Invalid template language")?,
                    content: content_map,
                })
            }
            _ => Err("Invalid template source type".into()),
        }
    }

    pub fn is_builtins(&self) -> bool {
        matches!(self, TemplateSource::Builtins)
    }

    pub fn is_shop(&self) -> bool {
        matches!(self, TemplateSource::Shop { .. })
    }

    pub fn is_custom(&self) -> bool {
        matches!(self, TemplateSource::Custom { .. })
    }
}

/// Information about the owner of a template
///
/// Note: there are two types of ownership:
/// - Authorship ownership (who created the template)
/// - Usage ownership (who is using the template in their guild)
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateOwner {
    User { id: UserId },
    Guild { id: GuildId },
}

#[allow(dead_code)]
impl TemplateOwner {
    /// Create a new TemplateOwner
    ///
    /// Returns [`None`] if the owner_type is invalid or the owner_id cannot be parsed
    pub fn new(owner_type: &str, owner_id: &str) -> Option<Self> {
        match owner_type {
            "user" => {
                let id = owner_id.parse().ok()?;
                Some(TemplateOwner::User { id })
            }
            "guild" => {
                let id = owner_id.parse().ok()?;
                Some(TemplateOwner::Guild { id })
            }
            _ => None,
        }
    }

    /// Returns if a guild owns this template
    pub fn guild_owns(&self, guild_id: GuildId) -> bool {
        matches!(self, TemplateOwner::Guild { id } if *id == guild_id)
    }

    /// Returns if a user owns this template
    pub fn user_owns(&self, user_id: UserId) -> bool {
        matches!(self, TemplateOwner::User { id } if *id == user_id)
    }

    /// Returns the owner type as a string
    pub fn owner_type(&self) -> &str {
        match self {
            TemplateOwner::User { .. } => "user",
            TemplateOwner::Guild { .. } => "guild",
        }
    }

    /// Returns the owner ID as a string
    pub fn owner_id(&self) -> String {
        match self {
            TemplateOwner::User { id } => id.to_string(),
            TemplateOwner::Guild { id } => id.to_string(),
        }
    }

    // Converts a TemplateOwner to a Id
    #[deprecated = "Remove this method once Id gets removed"]
    pub fn to_id(&self) -> Id {
        match self {
            TemplateOwner::Guild { id } => Id::GuildId(*id),
            // Note: Currently, only GuildId is supported in Id
            TemplateOwner::User { .. } => {
                panic!("TemplateOwner::User cannot be converted to Id (not yet implemented)")
            }
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AttachedTemplateId(Uuid);

impl AttachedTemplateId {
    fn new(id: Uuid) -> Self {
        Self(id)
    }

    pub fn id(&self) -> Uuid {
        self.0
    }

    /// Fetch the AttachedTemplate associated with this AttachedTemplateId
    pub async fn fetch<'c, E>(self, db: E) -> Result<Option<AttachedTemplate>, Error>
    where
        E: sqlx::Executor<'c, Database = Postgres>,
    {
        AttachedTemplate::fetch_by_id(db, self).await
    }
}

/// Represents an attached template
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachedTemplate {
    /// The ID of the template attachment
    pub id: AttachedTemplateId,

    /// The owner of the template usage wise
    pub owner: TemplateOwner,

    /// The template attachment source
    pub source: TemplateSource,

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

pub struct AttachedTemplateUpdate {
    pub id: AttachedTemplateId,
    pub allowed_caps: Option<Vec<String>>,
    pub events: Option<Vec<String>>,
    pub state: Option<TemplateState>,
    pub source: Option<TemplateSource>,
}

#[allow(dead_code)]
impl AttachedTemplate {
    /// Updates the templates data
    ///
    /// Dev note: this is the underlying DB code. Workers should use
    /// a dedicated API within WorkerCacheData or WorkerDB which will
    /// call this and handle/trigger cache regeneration etc as needed.
    pub async fn update<'c, E>(update: AttachedTemplateUpdate, db: E) -> Result<(), Error>
    where
        E: sqlx::Executor<'c, Database = Postgres>,
    {
        match update.source {
            Some(TemplateSource::Custom {
                name,
                language,
                content,
            }) => {
                sqlx::query(
                    r#"
                    UPDATE attached_templates
                    SET 
                        allowed_caps = COALESCE($1, allowed_caps),
                        events = COALESCE($2, events),
                        state = COALESCE($3, state),
                        last_updated_at = NOW(),
                        name = $4,
                        language = $5,
                        content = $6
                    WHERE id = $7 AND source = $8
                    "#,
                )
                .bind(&update.allowed_caps)
                .bind(&update.events)
                .bind(update.state.map(|s| s.to_string()))
                .bind(name)
                .bind(language.to_string())
                .bind(serde_json::to_value(&content)?)
                .bind(update.id.id())
                .bind("custom")
                .execute(db)
                .await?;
            }
            Some(TemplateSource::Shop { .. }) => {
                // TODO: Rethink if we want to allow updating shop references at all (even for staff)
                return Err("It is not allowed to update shop reference data of an attached template at this time".into());
            }
            Some(TemplateSource::Builtins) => {
                return Err("It is not allowed to explicitly set source to builtins for an update operation".into());
            }
            None => {
                sqlx::query(
                    r#"
                        UPDATE attached_templates
                        SET 
                            allowed_caps = COALESCE($1, allowed_caps),
                            events = COALESCE($2, events),
                            state = COALESCE($3, state),
                            last_updated_at = NOW(),
                        WHERE id = $4
                    "#,
                )
                .bind(&update.allowed_caps)
                .bind(&update.events)
                .bind(update.state.map(|s| s.to_string()))
                .bind(update.id.id())
                .execute(db)
                .await?;
            }
        }

        Ok(())
    }

    /// Deletes the guild template from the database
    ///
    /// Dev note: this is the underlying DB code. Workers should use
    /// a dedicated API within WorkerCacheData or WorkerDB which will
    /// call this and handle/trigger cache regeneration etc as needed.
    pub async fn delete<'c, E>(id: AttachedTemplateId, db: E) -> Result<(), Error>
    where
        E: sqlx::Executor<'c, Database = Postgres>,
    {
        sqlx::query(
            r#"
            DELETE FROM attached_templates
            WHERE id = $1
            "#,
        )
        .bind(id.id())
        .execute(db)
        .await?;

        Ok(())
    }
}

impl AttachedTemplate {
    /// Fetches all NormalAttachedTemplate's
    #[allow(dead_code)]
    pub async fn fetch_all<'c, E>(db: E) -> Result<Vec<Self>, Error>
    where
        E: sqlx::Executor<'c, Database = Postgres>,
    {
        let rows = sqlx::query(
            r#"
                SELECT 
                    id,
                    owner_type,
                    owner_id,
                    source,

                    -- data custom
                    name,
                    language,
                    content,

                    -- data shop
                    shop_ref,

                    -- metadata
                    created_at,
                    last_updated_at,
                    allowed_caps,
                    events,
                    state
                FROM attached_templates"#,
        )
        .fetch_all(db)
        .await?;

        let mut templates = Vec::new();
        for row in rows {
            let db_template = AttachedTemplateDb::try_from(row)?;
            let template = db_template.into_attached_template()?;
            templates.push(template);
        }

        Ok(templates)
    }

    /// Fetches all NormalAttachedTemplate's for a given owner
    #[allow(dead_code)]
    pub async fn fetch_all_for_owner<'c, E>(db: E, owner: TemplateOwner) -> Result<Vec<Self>, Error>
    where
        E: sqlx::Executor<'c, Database = Postgres>,
    {
        let rows = sqlx::query(
            r#"
                SELECT 
                    id,
                    owner_type,
                    owner_id,
                    source,

                    -- data custom
                    name,
                    language,
                    content,

                    -- data shop
                    shop_ref,

                    -- metadata
                    created_at,
                    last_updated_at,
                    allowed_caps,
                    events,
                    state
                FROM attached_templates 
                WHERE owner_type = $1
                AND owner_id = $2"#,
        )
        .bind(owner.owner_type())
        .bind(owner.owner_id())
        .fetch_all(db)
        .await?;

        let mut templates = Vec::new();
        for row in rows {
            let db_template = AttachedTemplateDb::try_from(row)?;
            let template = db_template.into_attached_template()?;
            templates.push(template);
        }

        Ok(templates)
    }

    /// Fetches a AttachedTemplate from the database
    /// by its id
    ///
    /// NOTE: This method does not check ownership,
    #[allow(dead_code)]
    pub async fn fetch_by_id<'c, E>(db: E, id: AttachedTemplateId) -> Result<Option<Self>, Error>
    where
        E: sqlx::Executor<'c, Database = Postgres>,
    {
        let row = sqlx::query(
            r#"
                SELECT 
                    id,
                    owner_type,
                    owner_id,
                    source,

                    -- data custom
                    name,
                    language,
                    content,

                    -- data shop
                    shop_ref,

                    -- metadata
                    created_at,
                    last_updated_at,
                    allowed_caps,
                    events,
                    state
                FROM attached_templates 
                WHERE id = $1"#,
        )
        .bind(id.id())
        .fetch_optional(db)
        .await?;

        if let Some(row) = row {
            let db_template = AttachedTemplateDb::try_from(row)?;
            let template = db_template.into_attached_template()?;
            Ok(Some(template))
        } else {
            Ok(None)
        }
    }
}

/// Simple intermediary struct for DB -> NormalAttachedTemplate conversion
struct AttachedTemplateDb {
    id: AttachedTemplateId,
    owner_type: String,
    owner_id: String,
    source: TemplateSource,
    created_at: DateTime<Utc>,
    last_updated_at: DateTime<Utc>,
    allowed_caps: Vec<String>,
    events: Vec<String>,
    state: TemplateState,
}

impl AttachedTemplateDb {
    /// Convert a (internal) AttachedTemplateDb to a NormalAttachedTemplate
    fn into_attached_template(self) -> Result<AttachedTemplate, Error> {
        Ok(AttachedTemplate {
            id: self.id,
            owner: TemplateOwner::new(&self.owner_type, &self.owner_id)
                .ok_or("Invalid template owner")?,
            source: self.source,
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
            id: AttachedTemplateId::new(row.try_get("id")?),
            owner_type: row.try_get("owner_type")?,
            owner_id: row.try_get("owner_id")?,
            source: TemplateSource::from_row(&row)?,
            created_at: row.try_get("created_at")?,
            last_updated_at: row.try_get("last_updated_at")?,
            allowed_caps: row.try_get("allowed_caps")?,
            events: row.try_get("events")?,
            state: {
                let state_str: String = row.try_get("state")?;
                TemplateState::from_str(&state_str)?
            },
        })
    }
}
