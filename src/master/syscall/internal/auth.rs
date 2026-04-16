use chrono::{DateTime, Duration, Utc};
use rand::distr::{Alphanumeric, SampleString};
use serenity::all::UserId;
use sqlx::PgPool;
use crate::master::syscall::{MSyscallError, MSyscallHandler, types::auth::UserSession};

/// The response from checking web auth
/// 
/// This enum can be used to control API access
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum AuthResponse {
    Success {
        user_id: UserId,
        session_id: String,
        state: String,
        session_type: String,
    },
    ApiBanned {
        user_id: UserId,
        session_id: String,
        session_type: String,
    },
    InvalidToken,
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
            user_id: auth.user_id.parse()?,
            session_id: auth.session_id.to_string(),
            session_type: auth.session_type,
        });
    }

    // If everything is fine, return the success response
    Ok(AuthResponse::Success {
        user_id: auth.user_id.parse()?,
        session_id: auth.session_id.to_string(),
        session_type: auth.session_type,
        state: user_state.state,
    })
}

/// Creates a new web user
pub async fn create_web_user_from_oauth2<'c, E>(executor: E, user_id: &str) -> Result<(), crate::Error> 
where E: sqlx::PgExecutor<'c> {
    // Insert the user into the database
    sqlx::query(
        "INSERT INTO users (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(&user_id)
    .execute(executor)
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
    AppLogin,
    Api {
        expires_at: DateTime<Utc>,
    }
}

/// 1 hour expiry time
const LOGIN_EXPIRY_TIME: Duration = Duration::seconds(3600);
/// 14 day expiry time for app logins
const APP_LOGIN_EXPIRY_TIME: Duration = Duration::days(14);

/// Create a new session for a web user
pub async fn create_web_session<'c, E>(
    executor: E,
    user_id: &str, 
    name: Option<String>,
    session_type: SessionType,
) -> Result<ICreatedWebSession, crate::Error> 
where E: sqlx::PgExecutor<'c> {
    // Generate a new session ID
    #[derive(sqlx::FromRow)]
    struct NewSession {
        session_id: uuid::Uuid,
    }

    let token = Alphanumeric.sample_string(&mut rand::rng(), 128);

    let (session_type, expiry) = match session_type {
        SessionType::Login => ("login", Utc::now() + LOGIN_EXPIRY_TIME),
        SessionType::AppLogin => ("app_login", Utc::now() + APP_LOGIN_EXPIRY_TIME),
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
    .fetch_one(executor)
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

/// Helper method to fetch user access token, refreshing it if needed
pub async fn get_user_access_token(handler: &MSyscallHandler, user_id: &str) -> Result<String, MSyscallError> {
    let mut tx = handler.pool.begin().await?;

    let (data, access_token_last_set) = OauthTokenResponse::get(&mut *tx, user_id).await?;

    if Utc::now() > access_token_last_set + Duration::seconds(data.expires_in as i64) {
        // expired
        #[derive(serde::Serialize)]
        pub struct Response {
            grant_type: &'static str,
            refresh_token: String
        }

        let resp = handler.reqwest.post(format!("{}/api/v10/oauth2/token", crate::CONFIG.meta.proxy))
            .form(&Response {
                grant_type: "refresh_token",
                refresh_token: data.refresh_token
            })
            .send()
            .await
            .map_err(|e| format!("Failed to get new access token: {e:?}"))?;

        if resp.status() != reqwest::StatusCode::OK {
            let error_text = resp.text().await?;
            return Err(format!("Failed to get new access token: {}", error_text).into());
        }

        let token_response: OauthTokenResponse = resp.json().await?;
        token_response.save(&mut *tx, user_id).await?;

        return Ok(token_response.access_token)
    }

    tx.commit().await?;

    Ok(data.access_token)
}

#[derive(serde::Deserialize)]
pub struct OauthTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: i32,
    pub scope: String,
}

impl OauthTokenResponse {
    /// Gets a oauth token response to database as well as when it was created
    pub async fn get<'c, E>(executor: E, user_id: &str) -> Result<(Self, DateTime<Utc>), MSyscallError> 
    where E: sqlx::PgExecutor<'c> 
    {
        #[derive(sqlx::FromRow)]
        struct AccessToken {
            access_token: String,
            access_token_last_set: DateTime<Utc>,
            refresh_token: String,
            access_token_expiry: i32,
            scope: String
        }

        let data = sqlx::query_as::<_, AccessToken>("SELECT access_token, access_token_last_set, refresh_token, access_token_expiry, scope FROM user_oauths WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(executor)
            .await?;

        let Some(data) = data else {
            return Err(MSyscallError::UserOauth2Needed);
        };
        Ok((Self {
            access_token: data.access_token,
            refresh_token: data.refresh_token,
            expires_in: data.access_token_expiry,
            scope: data.scope
        }, data.access_token_last_set))
    }

    /// Saves a oauth token response to database
    pub async fn save<'c, E>(&self, executor: E, user_id: &str) -> Result<(), MSyscallError> 
    where E: sqlx::PgExecutor<'c> 
    {
        sqlx::query("INSERT INTO user_oauths (user_id, access_token, refresh_token, access_token_expiry, access_token_last_set, scope)
        VALUES ($1, $2, $3, $4, NOW(), $5) ON CONFLICT (user_id) DO UPDATE SET access_token = EXCLUDED.access_token, refresh_token = EXCLUDED.refresh_token,
        access_token_last_set = EXCLUDED.access_token_last_set, access_token_expiry = EXCLUDED.access_token_expiry, scope = EXCLUDED.scope
        ")
        .bind(user_id)
        .bind(&self.access_token)
        .bind(&self.refresh_token)
        .bind(&self.expires_in)
        .bind(&self.scope)
        .execute(executor)
        .await?;
        Ok(())
    }
}