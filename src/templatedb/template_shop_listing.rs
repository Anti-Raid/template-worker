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
    /// Reference to the base template pool
    pub template_pool_ref: BaseTemplateRef,

    /// Short description of the shop listing
    pub short: String,

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
    template_pool_ref: Uuid,
    short: String,
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
            template_pool_ref: row.try_get("template_pool_ref")?,
            short: row.try_get("short")?,
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
        template_ref: BaseTemplateRef,
    ) -> Result<Option<TemplateShopListing>, Error> {
        let row = sqlx::query(r#"
            SELECT 
                template_pool_ref,
                short,  
                review_state, 
                default_events, 
                default_allowed_caps 
            FROM template_shop_listings WHERE id = $1"#
            )
            .bind(template_ref.id())
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
                template_pool_ref,
                short,  
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

impl BaseTemplateRef {
    /// Fetch the full TemplateShopListing from the database
    pub async fn fetch_shop_listings(
        self,
        pool: &sqlx::PgPool,
    ) -> Result<Option<TemplateShopListing>, Error> {
        TemplateShopListing::fetch_by_id(pool, self).await
    }
}