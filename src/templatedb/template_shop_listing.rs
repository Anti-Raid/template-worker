use std::{collections::HashMap, fmt::Display, str::FromStr};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;

use crate::{templatedb::attached_templates::TemplateLanguage, Error};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TemplateShopReviewState {
    #[serde(rename = "pending")]
    Pending,
    #[serde(rename = "approved")]
    Approved,
    #[serde(rename = "denied")]
    Denied,
}

impl FromStr for TemplateShopReviewState {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(TemplateShopReviewState::Pending),
            "approved" => Ok(TemplateShopReviewState::Approved),
            "denied" => Ok(TemplateShopReviewState::Denied),
            _ => Err("Invalid template state".into()),
        }
    }
}

impl Display for TemplateShopReviewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let state_str = match self {
            TemplateShopReviewState::Pending => "pending",
            TemplateShopReviewState::Approved => "approved",
            TemplateShopReviewState::Denied => "denied",
        };
        write!(f, "{}", state_str)
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ShopListingId(Uuid);

#[allow(dead_code)] // todo: remove this once the fetch api in iapi is added
impl ShopListingId {
    pub(super) fn new(id: Uuid) -> Self {
        Self(id)
    }

    pub fn id(&self) -> Uuid {
        self.0
    }

    /// Fetch the TemplateShopListing associated with this ShopListingId
    pub async fn fetch<'c, E>(self, db: E) -> Result<Option<TemplateShopListing>, Error>
    where
        E: sqlx::Executor<'c, Database = sqlx::Postgres>,
    {
        TemplateShopListing::fetch_by_id(db, self).await
    }
}

/// Template shop listings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateShopListing {
    /// ID of the shop listing
    pub id: ShopListingId,

    /// Name of the shop listing
    pub name: String,

    /// Short description of the shop listing
    pub short: String,

    /// The review state of the shop listing
    pub review_state: TemplateShopReviewState,

    /// Default events associated with this shop listing
    pub default_events: Vec<String>,

    /// Default allowed capabilities for this shop listing
    pub default_allowed_caps: Vec<String>,

    /// Content VFS (Language)
    pub language: TemplateLanguage,

    /// Content VFS
    pub content: HashMap<String, String>,

    /// When the shop listing was created
    pub created_at: DateTime<Utc>,

    /// When the shop listing was last updated
    pub last_updated_at: DateTime<Utc>,
}

/// Helper intermediary struct for DB -> TemplateShopListing conversion
struct TemplateShopListingDb {
    id: ShopListingId,
    name: String,
    short: String,
    review_state: String,
    default_events: Vec<String>,
    default_allowed_caps: Vec<String>,
    language: String,
    content: HashMap<String, String>,
    created_at: DateTime<Utc>,
    last_updated_at: DateTime<Utc>,
}

#[allow(dead_code)]
impl TemplateShopListingDb {
    /// Convert a (internal) TemplateShopListingDb to a TemplateShopListing
    fn into_template_shop_listing(self) -> Result<TemplateShopListing, Error> {
        let review_state = TemplateShopReviewState::from_str(&self.review_state)?;
        let language = TemplateLanguage::from_str(&self.language)?;

        Ok(TemplateShopListing {
            id: self.id,
            name: self.name,
            short: self.short,
            review_state,
            default_events: self.default_events,
            default_allowed_caps: self.default_allowed_caps,
            language,
            content: self.content,
            created_at: self.created_at,
            last_updated_at: self.last_updated_at,
        })
    }
}

impl TryFrom<PgRow> for TemplateShopListingDb {
    type Error = Error;
    fn try_from(row: PgRow) -> Result<Self, Self::Error> {
        Ok(TemplateShopListingDb {
            id: ShopListingId::new(row.try_get("id")?),
            name: row.try_get("name")?,
            short: row.try_get("short")?,
            review_state: row.try_get("review_state")?,
            default_events: row.try_get("default_events")?,
            default_allowed_caps: row.try_get("default_allowed_caps")?,
            language: row.try_get("language")?,
            content: {
                let content_json: serde_json::Value = row.try_get("content")?;
                let content_map: HashMap<String, String> = serde_json::from_value(content_json)?;
                content_map
            },
            created_at: row.try_get("created_at")?,
            last_updated_at: row.try_get("last_updated_at")?,
        })
    }
}

#[allow(dead_code)]
impl TemplateShopListing {
    /// Fetch a TemplateShopListing by its ID
    pub async fn fetch_by_id<'c, E>(
        db: E,
        id: ShopListingId,
    ) -> Result<Option<TemplateShopListing>, Error>
    where
        E: sqlx::Executor<'c, Database = sqlx::Postgres>,
    {
        let row = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                short,  
                review_state, 
                default_events, 
                default_allowed_caps,
                language,
                content,
                created_at, 
                last_updated_at
            FROM template_shop_listings 
            WHERE id = $1"#,
        )
        .bind(id.id())
        .fetch_optional(db)
        .await?;

        if let Some(row) = row {
            let db_listing = TemplateShopListingDb::try_from(row)?;
            let listing = db_listing.into_template_shop_listing()?;
            Ok(Some(listing))
        } else {
            Ok(None)
        }
    }

    /// Fetch all TemplateShopListings
    pub async fn fetch_all<'c, E>(db: E) -> Result<Vec<TemplateShopListing>, Error>
    where
        E: sqlx::Executor<'c, Database = sqlx::Postgres>,
    {
        let rows = sqlx::query(
            r#"
            SELECT 
                id,
                name,
                short,  
                review_state, 
                default_events, 
                default_allowed_caps,
                language,
                content,
                created_at, 
                last_updated_at
            FROM template_shop_listings
            ORDER BY created_at DESC"#,
        )
        .fetch_all(db)
        .await?;
        let mut listings = Vec::new();
        for row in rows {
            let db_listing = TemplateShopListingDb::try_from(row)?;
            let listing = db_listing.into_template_shop_listing()?;
            listings.push(listing);
        }
        Ok(listings)
    }
}
