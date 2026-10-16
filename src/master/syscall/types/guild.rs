use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct GuildLockdown { pub id: String, pub r#type: Option<String>, pub reason: Option<String>, pub created_at: DateTime<Utc> }

#[derive(Debug, Serialize, Deserialize)]
pub struct GuildBackup {
    pub id: String,
    pub created_by: Option<String>,
    pub size: Option<i64>,
    pub created_at: DateTime<Utc>, pub last_updated_at: DateTime<Utc>,
}
