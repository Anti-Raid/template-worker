use chrono::{DateTime, Duration, Utc};
use rand::distr::{Alphanumeric, SampleString};
use sqlx::PgPool;

use crate::api::types::UserSession;

/// The response from checking web auth
/// 
/// This enum can be used to control API access
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AuthResponse {
    Success {
        user_id: String,
        session_id: String,
        state: String,
        session_type: String,
    },
    ApiBanned {
        user_id: String,
        session_id: String,
        session_type: String,
    },
    InvalidToken,
}

impl AuthResponse {
    /// Returns true if the response is a success
    #[allow(dead_code)]
    pub fn is_success(&self) -> bool {
        matches!(self, AuthResponse::Success { .. })
    }

    /// Returns true if the response is an API banned response
    #[allow(dead_code)]
    pub fn is_api_banned(&self) -> bool {
        matches!(self, AuthResponse::ApiBanned { .. })
    }

    /// Returns true if the response is an invalid token response
    #[allow(dead_code)]
    pub fn is_invalid_token(&self) -> bool {
        matches!(self, AuthResponse::InvalidToken)
    }
}

pub async fn check_web_auth(
    pool: &PgPool,
    token: &str,
) -> Result<AuthResponse, crate::Error> {
    // Delete old/expiring auths first
    sqlx::query("DELETE FROM web_api_tokens WHERE expiry < NOW()")
        .execute(pool)
        .await?;

    // Check if the user exists with said API token
    #[derive(sqlx::FromRow)]
    struct UserAuth {
        user_id: String,
        session_id: uuid::Uuid,
        session_type: String,
    }

    let user_auth: Option<UserAuth> = sqlx::query_as(
        "SELECT user_id, id AS session_id, type AS session_type FROM web_api_tokens WHERE token = $1",
    )
    .bind(token)
    .fetch_optional(pool)
    .await?;

    let Some(auth) = user_auth else {
        return Ok(AuthResponse::InvalidToken);
    };

    // Check if the user is banned
    #[derive(sqlx::FromRow)]
    struct UserState {
        state: String,
    }

    let user_state: UserState = sqlx::query_as(
        "SELECT state FROM users WHERE user_id = $1",
    )
    .bind(auth.user_id.clone())
    .fetch_one(pool)
    .await?;

    if user_state.state == "banned" {
        return Ok(AuthResponse::ApiBanned {
            user_id: auth.user_id,
            session_id: auth.session_id.to_string(),
            session_type: auth.session_type,
        });
    }

    // If everything is fine, return the success response
    Ok(AuthResponse::Success {
        user_id: auth.user_id,
        session_id: auth.session_id.to_string(),
        session_type: auth.session_type,
        state: user_state.state,
    })
}

/// Creates a new web user
pub async fn create_web_user_from_oauth2(pool: &PgPool, user_id: &str, access_token: &str) -> Result<(), crate::Error> {
    // Insert the user into the database
    sqlx::query(
        "INSERT INTO users (user_id, access_token) VALUES ($1, $2) ON CONFLICT (user_id) DO UPDATE SET access_token = EXCLUDED.access_token",
    )
    .bind(&user_id)
    .bind(access_token)
    .execute(pool)
    .await?;

    Ok(())
}

pub struct ICreatedWebSession {
    pub session_id: String,
    pub token: String,
    pub expires_at: DateTime<Utc>
}

pub enum SessionType {
    Login,
    Api {
        expires_at: DateTime<Utc>,
    }
}

/// 1 hour expiry time
const LOGIN_EXPIRY_TIME: Duration = Duration::seconds(3600);

/// Create a new session for a web user
pub async fn create_web_session(
    pool: &PgPool, 
    user_id: &str, 
    name: Option<String>,
    session_type: SessionType,
) -> Result<ICreatedWebSession, crate::Error> {
    // Generate a new session ID
    #[derive(sqlx::FromRow)]
    struct NewSession {
        session_id: uuid::Uuid,
    }

    let token = Alphanumeric.sample_string(&mut rand::rng(), 128);

    let (session_type, expiry) = match session_type {
        SessionType::Login => ("login", Utc::now() + LOGIN_EXPIRY_TIME),
        SessionType::Api { expires_at } => ("api", expires_at),
    };

    let new_session: NewSession = sqlx::query_as(
        "INSERT INTO web_api_tokens (user_id, type, token, expiry, name) VALUES ($1, $2, $3, $4, $5) RETURNING id AS session_id",
    )
    .bind(user_id)
    .bind(session_type)
    .bind(&token)
    .bind(expiry)
    .bind(name)
    .fetch_one(pool)
    .await?;

    Ok(ICreatedWebSession { 
        session_id: new_session.session_id.to_string(),
        token,
        expires_at: expiry,
    })
}

/// Returns the list of all sessions for a user
pub async fn get_user_sessions(pool: &PgPool, user_id: &str) -> Result<Vec<UserSession>, crate::Error> {
    #[derive(sqlx::FromRow)]
    pub struct UserSessionRow {
        pub id: uuid::Uuid,
        pub name: Option<String>,
        pub user_id: String,
        pub created_at: DateTime<Utc>,
        pub typ: String,
        pub expiry: DateTime<Utc>,
    }

    let sessions: Vec<UserSessionRow> = sqlx::query_as(
        "SELECT id, name, user_id, created_at, type AS typ, expiry FROM web_api_tokens WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    let user_sessions = sessions.into_iter().map(|s| UserSession {
        id: s.id.to_string(),
        name: s.name,
        user_id: s.user_id,
        created_at: s.created_at,
        r#type: s.typ,
        expiry: s.expiry,
    }).collect();

    Ok(user_sessions)
}

pub async fn delete_user_session(pool: &PgPool, user_id: &str, session_id: &str) -> Result<(), crate::Error> {
    let session_id_uuid = match uuid::Uuid::parse_str(session_id) {
        Ok(uuid) => uuid,
        Err(_) => return Err("Invalid session ID format".into()),
    };
    
    let res = sqlx::query("DELETE FROM web_api_tokens WHERE user_id = $1 AND id = $2")
        .bind(user_id)
        .bind(session_id_uuid)
        .execute(pool)
        .await?;

    if res.rows_affected() == 0 {
        return Err("No session found to delete".into());
    }

    Ok(())
}