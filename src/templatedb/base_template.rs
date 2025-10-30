use std::{collections::HashMap, str::FromStr};

use serenity::all::{GuildId, UserId};
use sqlx::{Executor, Postgres, Row, postgres::PgRow};
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::Error;

/// Information about the owner of a template
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemplateOwner {
    User {
        id: UserId,
    },
    Guild {
        id: GuildId,
    },
}

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
}

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

/// Template state
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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

/// Base/common template data
/// 
/// Internally stored in a 'template pool' and then referenced where needed
#[derive(Debug, PartialEq)]
pub struct BaseTemplate {
    /// Identifier for the template in the pool
    pub id: Uuid,
 
    /// Name of the template
    pub name: String,

    /// Owner data
    pub owner: TemplateOwner,

    /// Language of the template
    pub language: String,
    
    /// Content of the template (VFS)
    pub content: HashMap<String, String>,

    /// When the template was last updated at
    pub last_updated_at: DateTime<Utc>,

    /// When the template was created at
    pub created_at: DateTime<Utc>,

    /// State of the template
    pub state: TemplateState,
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
    state: String,
}

impl BaseTemplateDb {
    /// Convert a (internal) BaseTemplateDb to a BaseTemplate
    fn into_base_template(self) -> Result<BaseTemplate, Error> {
        let owner = TemplateOwner::new(&self.owner_type, &self.owner_id)
            .ok_or(format!("Invalid owner type or id: {} {}", self.owner_type, self.owner_id))?;

        let state = TemplateState::from_str(&self.state)?;

        let content_map: HashMap<String, String> = serde_json::from_value(self.content)
            .map_err(|_| "Failed to parse template content")?;

        Ok(BaseTemplate {
            id: self.id,
            name: self.name,
            owner,
            language: self.language,
            content: content_map,
            last_updated_at: self.last_updated_at,
            created_at: self.created_at,
            state,
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
            state: row.try_get("state")?,
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
                created_at,
                state
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
                created_at,
                state
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
}

/// Simple ergonomic struct that points to a BaseTemplate in the DB by ID
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BaseTemplateRef {
    /// ID of the BaseTemplate
    id: Uuid,
}

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
}