use sqlx::postgres::PgRow;
use sqlx::Row;
use uuid::Uuid;
use chrono::{DateTime, Utc};

use crate::Error;

use super::base_template::BaseTemplateRef;

pub enum TemplateShopReviewState {
    Pending,
    Approved,
    Denied,
}

/// Template shop listings
pub struct TemplateShopListing {
    /// The unique ID of the shop listing
    pub id: Uuid,

    /// Short description of the shop listing
    pub short: String,

    /// Reference to the base template pool
    pub template_pool_ref: BaseTemplateRef,

    /// The review state of the shop listing
    pub review_state: TemplateShopReviewState,

    /// Default events associated with this shop listing
    pub default_events: Vec<String>,

    /// Default allowed capabilities for this shop listing
    pub default_allowed_caps: Vec<String>,

    /// When the shop listing was created
    pub created_at: DateTime<Utc>,

    /// When the shop listing was last updated
    pub last_updated_at: DateTime<Utc>,
}

/// Helper intermediary struct for DB -> TemplateShopListing conversion
struct TemplateShopListingDb {
    id: Uuid,
    short: String,
    template_pool_ref: Uuid,
    review_state: String,
    default_events: Vec<String>,
    default_allowed_caps: Vec<String>,
    created_at: DateTime<Utc>,
    last_updated_at: DateTime<Utc>,
}

impl TemplateShopListingDb {
    /// Convert a (internal) TemplateShopListingDb to a TemplateShopListing
    fn into_template_shop_listing(self) -> Result<TemplateShopListing, Error> {
        let review_state = match self.review_state.as_str() {
            "pending" => TemplateShopReviewState::Pending,
            "approved" => TemplateShopReviewState::Approved,
            "denied" => TemplateShopReviewState::Denied,
            _ => return Err(format!("Invalid review state: {}", self.review_state).into()),
        };

        Ok(TemplateShopListing {
            id: self.id,
            short: self.short,
            template_pool_ref: BaseTemplateRef::new(self.template_pool_ref),
            review_state,
            default_events: self.default_events,
            default_allowed_caps: self.default_allowed_caps,
            created_at: self.created_at,
            last_updated_at: self.last_updated_at,
        })
    }
}

impl TryFrom<PgRow> for TemplateShopListingDb {
    type Error = Error;
    fn try_from(row: PgRow) -> Result<Self, Self::Error> {
        Ok(TemplateShopListingDb {
            id: row.try_get("id")?,
            short: row.try_get("short")?,
            template_pool_ref: row.try_get("template_pool_ref")?,
            review_state: row.try_get("review_state")?,
            default_events: row.try_get("default_events")?,
            default_allowed_caps: row.try_get("default_allowed_caps")?,
            created_at: row.try_get("created_at")?,
            last_updated_at: row.try_get("last_updated_at")?,
        })
    }
}

impl TemplateShopListing {
    /// Fetch a TemplateShopListing by its ID
    pub async fn fetch_by_id(
        pool: &sqlx::PgPool,
        id: Uuid,
    ) -> Result<Option<TemplateShopListing>, Error> {
        let row = sqlx::query(r#"
            SELECT 
                id, 
                short, 
                template_pool_ref, 
                review_state, 
                default_events, 
                default_allowed_caps 
            FROM template_shop_listings WHERE id = $1"#
            )
            .bind(id)
            .fetch_optional(pool)
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
    pub async fn fetch_all(
        pool: &sqlx::PgPool,
    ) -> Result<Vec<TemplateShopListing>, Error> {
        let rows = sqlx::query(r#"
            SELECT 
                id, 
                short, 
                template_pool_ref, 
                review_state, 
                default_events, 
                default_allowed_caps, 
                created_at, 
                last_updated_at 
            FROM template_shop_listings
            ORDER BY created_at DESC"#
            )
            .fetch_all(pool)
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

/// Simple ergonomic struct that points to a TemplateShopListing in the DB by ID
pub struct TemplateShopListingRef {
    /// The unique ID of the shop listing
    id: Uuid,
}

impl TemplateShopListingRef {
    /// Creates a new TemplateShopListingRef
    /// by ID
    pub fn new(id: Uuid) -> Self {
        TemplateShopListingRef { id }
    }

    /// Returns the underlying ID of the TemplateShopListing
    pub fn id(&self) -> Uuid {
        self.id
    }

    /// Fetch the full TemplateShopListing from the database
    pub async fn fetch_from_db(
        &self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<TemplateShopListing>, Error> {
        TemplateShopListing::fetch_by_id(pool, self.id).await
    }
}