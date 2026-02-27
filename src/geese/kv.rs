use khronos_runtime::utils::khronos_value::KhronosValue;
use rand::distr::{Alphanumeric, SampleString};
use crate::worker::workervmmanager::Id;
use sqlx::Row;
use khronos_runtime::traits::ir::KvRecord;

#[derive(Clone)]
/// A simple wrapper around the database pool that provides just the key-value storage functionality for tenants
pub struct KeyValueDb {
    pool: sqlx::PgPool,
}

impl KeyValueDb {
    pub fn new(pool: sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Gets a key-value record for a given tenant ID, scopes, and key
    pub async fn kv_get(&self, tid: Id, mut scopes: Vec<String>, key: String) -> Result<Option<SerdeKvRecord>, crate::Error> {
        scopes.sort();
        
        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        let rec = sqlx::query(
            "SELECT id, scopes, value, created_at, last_updated_at FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND key = $3 AND scopes @> $4",
            )
            .bind(tid.tenant_id())
            .bind(tid.tenant_type())
            .bind(&key)
            .bind(scopes)
            .fetch_optional(&self.pool)
            .await?;

        let Some(rec) = rec else {
            return Ok(None);
        };

        Ok(Some(SerdeKvRecord {
            id: rec.try_get::<String, _>("id")?,
            key,
            scopes: rec.try_get::<Vec<String>, _>("scopes")?,
            value: {
                let value = rec
                    .try_get::<Option<serde_json::Value>, _>("value")?
                    .unwrap_or(serde_json::Value::Null);

                serde_json::from_value(value)
                    .map_err(|e| format!("Failed to deserialize value: {}", e))?
            },
            created_at: Some(rec.try_get("created_at")?),
            last_updated_at: Some(rec.try_get("last_updated_at")?),
        }))
    }

    pub async fn kv_list_scopes(&self, id: Id) -> Result<Vec<String>, crate::Error> {
        let rec = sqlx::query(
            "SELECT DISTINCT unnest_scope AS scope
FROM tenant_kv, unnest(scopes) AS unnest_scope
WHERE owner_id = $1
AND owner_type = $2
ORDER BY scope",
        )
        .bind(id.tenant_id())
        .bind(id.tenant_type())
        .fetch_all(&self.pool)
        .await?;

        let mut scopes = vec![];

        for rec in rec {
            scopes.push(rec.try_get("scope")?);
        }

        Ok(scopes)
    }

    pub async fn kv_set(
        &self,
        tid: Id,
        mut scopes: Vec<String>,
        key: String,
        data: KhronosValue,
    ) -> Result<(), crate::Error> {
        scopes.sort();

        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        let id = Alphanumeric.sample_string(&mut rand::rng(), 64);
        sqlx::query(
            "INSERT INTO tenant_kv (id, owner_id, owner_type, key, value, scopes) VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (owner_id, owner_type, key, scopes) DO UPDATE SET value = EXCLUDED.value, last_updated_at = NOW()",
        )
        .bind(&id)
        .bind(tid.tenant_id())
        .bind(tid.tenant_type())
        .bind(key)
        .bind(serde_json::to_value(data)?)
        .bind(scopes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn kv_delete(
        &self,
        tid: Id,
        mut scopes: Vec<String>,
        key: String,
    ) -> Result<(), crate::Error> {
        scopes.sort();

        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        sqlx::query(
        "DELETE FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND key = $3 AND scopes @> $4",
        )
        .bind(tid.tenant_id())
        .bind(tid.tenant_type())
        .bind(key)
        .bind(scopes)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn kv_find(
        &self,
        tid: Id,
        mut scopes: Vec<String>,
        query: String,
    ) -> Result<Vec<SerdeKvRecord>, crate::Error> {
        scopes.sort();

        // Shouldn't happen but scopes must be non-empty
        if scopes.is_empty() {
            return Err("Scopes cannot be empty".into());
        }

        let rec = {
            if query == "%%" {
                // Fast path, omit ILIKE if '%%' is used
                sqlx::query(
                "SELECT id, key, value, created_at, last_updated_at, scopes FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND scopes @> $3",
                )
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(scopes)
                .fetch_all(&self.pool)
                .await?
            } else {
                // with query
                sqlx::query(
                "SELECT id, key, value, created_at, last_updated_at, scopes FROM tenant_kv WHERE owner_id = $1 AND owner_type = $2 AND scopes @> $3 AND key LIKE $4",
                )
                .bind(tid.tenant_id())
                .bind(tid.tenant_type())
                .bind(scopes)
                .bind(query)
                .fetch_all(&self.pool)
                .await?
            }
        };

        let mut records = vec![];

        for rec in rec {
            let record = SerdeKvRecord {
                id: rec.try_get::<String, _>("id")?,
                scopes: rec.try_get::<Vec<String>, _>("scopes")?,
                key: rec.try_get("key")?,
                value: {
                    let rec = rec
                        .try_get::<Option<serde_json::Value>, _>("value")?
                        .unwrap_or(serde_json::Value::Null);

                    serde_json::from_value(rec)
                        .map_err(|e| format!("Failed to deserialize value: {}", e))?
                },
                created_at: Some(rec.try_get("created_at")?),
                last_updated_at: Some(rec.try_get("last_updated_at")?),
            };

            records.push(record);
        }

        Ok(records)
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct SerdeKvRecord {
    pub id: String,
    pub key: String,
    pub value: KhronosValue,
    pub scopes: Vec<String>,
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl Into<KvRecord> for SerdeKvRecord {
    fn into(self) -> KvRecord {
        KvRecord {
            id: self.id,
            key: self.key,
            value: self.value,
            scopes: self.scopes,
            created_at: self.created_at,
            last_updated_at: self.last_updated_at,
        }
    }
}