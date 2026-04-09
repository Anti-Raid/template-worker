use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSession {
    /// The ID of the session
    pub id: String,
    /// The name of the session
    pub name: Option<String>,
    /// The ID of the user who created the session
    pub user_id: String,
    /// The time the session was created
    pub created_at: DateTime<Utc>,
    /// The type of session (e.g., "login", "api")
    pub r#type: String,
    /// The time the session expires
    pub expiry: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Create a API user session
pub struct CreateUserSession {
    pub name: String,
    pub r#type: String, // Currently must be 'api'
    pub expiry: i64, // Expiry in seconds
}
