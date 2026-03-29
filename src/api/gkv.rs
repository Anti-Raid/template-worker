use chrono::{DateTime, Utc};
use khronos_runtime::utils::khronos_value::KhronosValue;
use serde::{Serialize, Deserialize};

#[derive(Clone)]
pub struct ApiPartialGkvFetcher {
    pool: sqlx::PgPool,
}

impl ApiPartialGkvFetcher {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    pub async fn global_kv_find(&self, scope: String, query: String) -> Result<Vec<PartialGlobalKv>, crate::Error> {
        let items: Vec<PartialGlobalKv> = if query == "%%" {
            sqlx::query_as(
                "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved'"
            )
            .bind(scope)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as(
                "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved' AND key LIKE $2"
            )
            .bind(scope)
            .bind(query)
            .fetch_all(&self.pool)
            .await?
        };

        Ok(items)
    }
    
    pub async fn global_kv_get(&self, key: String, version: i32, scope: String) -> Result<Option<PartialGlobalKv>, crate::Error> {
        let item: Option<PartialGlobalKv> = sqlx::query_as(
            "SELECT key, version, owner_id, owner_type, short, long, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE review_state = 'approved' AND key = $1 AND version = $2 AND scope = $3",
        )
        .bind(&key)
        .bind(version)
        .bind(&scope)
        .fetch_optional(&self.pool)
        .await?;

        let Some(mut gkv) = item else {
            return Ok(None);
        };

        if gkv.price.is_none() && gkv.public_data {
            let data = sqlx::query_as(
                "SELECT data FROM global_kv WHERE review_state = 'approved' AND key = $1 AND version = $2 AND scope = $3",
            )
            .bind(&key)
            .bind(version)
            .bind(scope)
            .fetch_optional(&self.pool)
            .await?;

            gkv.data = data;
        }

        Ok(Some(gkv))
    }
}

#[derive(Debug, Serialize, Deserialize, utoipa::ToSchema, sqlx::FromRow)]
pub struct PartialGlobalKv {
    pub key: String,
    pub version: i32,
    pub owner_id: String,
    pub owner_type: String,
    pub price: Option<i64>, // will only be set for shop items, otherwise None
    pub short: String, // short description for the key-value.
    #[schema(value_type = Object)]
    #[sqlx(json)]
    pub public_metadata: KhronosValue, // public metadata about the key-value
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub last_updated_at: DateTime<Utc>,
    pub public_data: bool,
    pub review_state: String,

    #[sqlx(default)]
    pub long: Option<String>, // long description for the key-value.

    #[sqlx(skip)]
    #[schema(value_type = Option<Object>)]
    pub data: Option<GlobalKvData>,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct GlobalKvData {
    #[sqlx(json)]
    pub data: KhronosValue, // the actual value of the key-value, may be private
}
