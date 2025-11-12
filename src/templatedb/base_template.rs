use std::{collections::HashMap, str::FromStr};

use serde::{Deserialize, Serialize};
use serenity::all::{GuildId, UserId};
use sqlx::{Executor, Postgres, Row, postgres::PgRow};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::{Error, worker::workervmmanager::Id};

/// Information about the owner of a template
/// 
/// Note: there are two types of ownership:
/// - Authorship ownership (who created the template)
/// - Usage ownership (who is using the template in their guild)
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateOwner {
    User {
        id: UserId,
    },
    Guild {
        id: GuildId,
    },
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

    /// Converts a TemplateOwner to a Id
    pub fn to_id(&self) -> Id {
        match self {
            TemplateOwner::Guild { id } => Id::GuildId(*id),
            // Note: Currently, only GuildId is supported in Id
            TemplateOwner::User { .. } => panic!("TemplateOwner::User cannot be converted to Id (not yet implemented)"),
        }
    }
}

/// What does the template reference?
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TemplateReference {
    Usage {
        owner: TemplateOwner,
    },
    ShopListing,
}

#[allow(dead_code)]
pub enum TemplateLanguage {
    Luau,
}

impl FromStr for TemplateLanguage {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "luau" => Ok(TemplateLanguage::Luau),
            _ => Err("Invalid template language".into()),
        }
    }
}

/// Base/common template data
/// 
/// Internally stored in a 'template pool' and then referenced where needed
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct BaseTemplate {
    /// Identifier for the template in the pool
    pub id: BaseTemplateRef,
 
    /// Name of the template
    pub name: String,

    /// Provides information about template ownership
    /// at a authorship level (authorship ownership)
    /// 
    /// For usage ownership, see AttachedTemplate::owner
    pub owner: TemplateOwner,

    /// Language of the template
    pub language: String,
    
    /// Content of the template (VFS)
    pub content: HashMap<String, String>,

    /// When the template was created at
    pub created_at: DateTime<Utc>,

    /// When the template was last updated at
    pub last_updated_at: DateTime<Utc>,
}

/// Helper intermediary struct for DB -> BaseTemplate conversion
struct BaseTemplateDb {
    id: Uuid,
    name: String,
    owner_type: String,
    owner_id: String,
    language: String,
    content: serde_json::Value,
    last_updated_at: DateTime<Utc>,
    created_at: DateTime<Utc>,
}

impl BaseTemplateDb {
    /// Convert a (internal) BaseTemplateDb to a BaseTemplate
    fn into_base_template(self) -> Result<BaseTemplate, Error> {
        let owner = TemplateOwner::new(&self.owner_type, &self.owner_id)
            .ok_or(format!("Invalid owner type or id: {} {}", self.owner_type, self.owner_id))?;

        let content_map: HashMap<String, String> = serde_json::from_value(self.content)
            .map_err(|_| "Failed to parse template content")?;

        Ok(BaseTemplate {
            id: BaseTemplateRef::new(self.id),
            name: self.name,
            owner,
            language: self.language,
            content: content_map,
            last_updated_at: self.last_updated_at,
            created_at: self.created_at,
        })
    }
}

impl TryFrom<PgRow> for BaseTemplateDb {
    type Error = Error;
    fn try_from(row: PgRow) -> Result<Self, Self::Error> {
        Ok(BaseTemplateDb {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            owner_type: row.try_get("owner_type")?,
            owner_id: row.try_get("owner_id")?,
            language: row.try_get("language")?,
            content: row.try_get("content")?,
            last_updated_at: row.try_get("last_updated_at")?,
            created_at: row.try_get("created_at")?,
        })
    }
}

impl BaseTemplate {
    /// Given an ID, fetch the BaseTemplate from the database
    pub async fn fetch_by_id<'c, E>(db: E, id: Uuid) -> Result<Option<Self>, Error> 
        where E: Executor<'c, Database = Postgres>
    {
        let record = sqlx::query(
            r#"
            SELECT
                id,
                name,
                owner_type,
                owner_id,
                language,
                content,
                last_updated_at,
                created_at
            FROM template_pool
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(db)
        .await?;

        if let Some(db_template) = record {
            let base_template_db: BaseTemplateDb = db_template.try_into()?;
            let base_template = base_template_db.into_base_template()?;
            Ok(Some(base_template))
        } else {
            Ok(None)
        }
    }

    /// Fetch all BaseTemplates from the database
    pub async fn fetch_all<'c, E>(db: E) -> Result<Vec<Self>, Error> 
        where E: Executor<'c, Database = Postgres>
    {
        let record = sqlx::query(
            r#"
            SELECT
                id,
                name,
                owner_type,
                owner_id,
                language,
                content,
                last_updated_at,
                created_at
            FROM template_pool
            "#,
        )
        .fetch_all(db)
        .await?;

        let mut templates = Vec::with_capacity(record.len());
        for db_template in record {
            let base_template_db: BaseTemplateDb = db_template.try_into()?;
            let base_template = base_template_db.into_base_template()?;
            templates.push(base_template);
        }

        Ok(templates)
    }

    /// Given an ID, fetch the BaseTemplate from the database
    pub async fn fetch_by_ids<'c, E>(db: E, ids: Vec<Uuid>) -> Result<Vec<Self>, Error> 
        where E: Executor<'c, Database = Postgres>
    {
        let record = sqlx::query(
            r#"
            SELECT
                id,
                name,
                owner_type,
                owner_id,
                language,
                content,
                last_updated_at,
                created_at
            FROM template_pool
            WHERE id = ANY($1)
            "#,
        )
        .bind(ids)
        .fetch_all(db)
        .await?;

        let mut templates = Vec::with_capacity(record.len());
        for db_template in record {
            let base_template_db: BaseTemplateDb = db_template.try_into()?;
            let base_template = base_template_db.into_base_template()?;
            templates.push(base_template);
        }

        Ok(templates)
    }

    /// Returns the references to the base template
    pub async fn template_refs<'c>(template_pool_ref: BaseTemplateRef, db: &mut sqlx::Transaction<'c, Postgres>) -> Result<Vec<TemplateReference>, Error> {
        let mut refs = Vec::new();

        // Add refs from other uses of this template attached
        let rows = sqlx::query(r#"
            SELECT owner_type, owner_id 
            FROM attached_templates 
            WHERE template_pool_ref = $1"#
        )
        .bind(template_pool_ref.id())
        .fetch_all(&mut (**db))
        .await?;

        for row in rows {
            let owner_type: String = row.try_get("owner_type")?;
            let owner_id: String = row.try_get("owner_id")?;
            let Some(owner) = TemplateOwner::new(&owner_type, &owner_id) else {
                log::warn!("Invalid owner data in attached_templates for template_pool_ref {}", template_pool_ref.id());
                continue;
            };
            refs.push(TemplateReference::Usage {
                owner,
            });
        }

        // Look for a shop listing
        let shop_listing_row = sqlx::query(r#"
            SELECT COUNT(*) FROM template_shop_listings
            WHERE template_pool_ref = $1
            "#
        )
        .bind(template_pool_ref.id())
        .fetch_one(&mut (**db))
        .await?;

        let listing_count: i64 = shop_listing_row.try_get(0)?;
        if listing_count > 0 {
            refs.push(TemplateReference::ShopListing);
        }

        Ok(refs)
    }
}

/// Simple ergonomic struct that points to a BaseTemplate in the DB by ID
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq, Serialize, Deserialize)]
pub struct BaseTemplateRef {
    /// ID of the BaseTemplate
    id: Uuid,
}

#[allow(dead_code)]
impl BaseTemplateRef {
    /// Creates a new BaseTemplateRef
    /// by ID
    pub fn new(id: Uuid) -> Self {
        BaseTemplateRef { id }
    }

    /// Returns the underlying ID of the BaseTemplate
    pub fn id(self) -> Uuid {
        self.id
    }

    /// Fetch the full BaseTemplate from the database
    pub async fn fetch_from_db<'c, E>(self, db: E) -> Result<Option<BaseTemplate>, Error> 
        where E: Executor<'c, Database = Postgres>
    {
        BaseTemplate::fetch_by_id(db, self.id).await
    }

    /// Returns the owner of the BaseTemplate
    pub async fn fetch_owner<'c, E>(self, db: E) -> Result<Option<TemplateOwner>, Error> 
        where E: Executor<'c, Database = Postgres> 
    {
        let record = sqlx::query(
            r#"
            SELECT
                owner_type,
                owner_id
            FROM template_pool
            WHERE id = $1
            "#,
        )
        .bind(self.id)
        .fetch_optional(db)
        .await?;

        if let Some(row) = record {
            let owner_type: String = row.try_get("owner_type")?;
            let owner_id: String = row.try_get("owner_id")?;

            let owner = TemplateOwner::new(&owner_type, &owner_id)
                .ok_or(format!("Invalid owner type or id: {} {}", owner_type, owner_id))?;

            Ok(Some(owner))
        } else {
            Ok(None)
        }
    }

    /// Returns the references to the base template
    pub async fn template_refs<'c>(self, db: &mut sqlx::Transaction<'c, Postgres>) -> Result<Vec<TemplateReference>, Error> {
        let refs = BaseTemplate::template_refs(self, db).await?;
        Ok(refs)
    }

    /// Garbage collect the template if it has no references
    pub async fn gc<'c>(self, db: &mut sqlx::Transaction<'c, Postgres>) -> Result<(), Error> {
        let refs = self.template_refs(db).await?;
        if refs.is_empty() {
            // No references, delete the base template
            sqlx::query(
                r#"
                DELETE FROM template_pool
                WHERE id = $1
                "#,
            )
            .bind(self.id())
            .execute(&mut (**db))
            .await?;
        }

        Ok(())
    }
}