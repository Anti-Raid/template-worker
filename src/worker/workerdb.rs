use std::{collections::HashMap, sync::Arc};
use chrono::{Utc, DateTime};

use super::workerstate::WorkerState;
use super::workervmmanager::Id;
use super::template::Template;
use super::builtins::{BUILTINS_NAME, USE_BUILTINS, BUILTINS};

type ArcVec<T> = Arc<Vec<Arc<T>>>;

#[derive(Debug)]
pub struct KeyExpiry {
    pub id: String,
    pub key: String,
    pub scopes: Vec<String>,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct KeyResume {
    pub id: String,
    pub key: String,
    pub scopes: Vec<String>,
}

/// WorkerDB provides database related code to the worker system
#[derive(Clone)]
pub struct WorkerDB {
    state: WorkerState
}

impl WorkerDB {
    pub fn new(state: WorkerState) -> Self {
        WorkerDB { state }
    }

    /// Returns all templates for all guilds in the database
    pub async fn get_templates(&self) -> Result<HashMap<Id, ArcVec<Template>>, crate::Error> {
        #[derive(sqlx::FromRow)]
        struct GuildTemplatePartial {
            guild_id: String,
        }

        let partials: Vec<GuildTemplatePartial> =
            sqlx::query_as("SELECT guild_id FROM guild_templates GROUP BY guild_id")
            .fetch_all(&self.state.pool)
            .await?;

        // TODO: Optimize to do one single query
        let mut templates = HashMap::with_capacity(partials.len());
        for partial in partials {
            let guild_id = partial.guild_id.parse()?;

            templates.insert(Id::GuildId(guild_id), self.get_templates_for(Id::GuildId(guild_id)).await?);
        }

        Ok(templates)
    }

    /// Gets all templates for a tenant from the database
    pub async fn get_templates_for(&self, id: Id) -> Result<ArcVec<Template>, crate::Error> {
        match id {
            Id::GuildId(guild_id) => {
                let mut templates_vec = Template::guild(guild_id, &self.state.pool)
                    .await?
                    .into_iter()
                    .map(|template| Arc::new(template))
                    .collect::<Vec<_>>();

                if USE_BUILTINS {
                    let mut found_base = false;
                    for template in templates_vec.iter() {
                        if template.name == BUILTINS_NAME {
                            found_base = true;
                            break;
                        }
                    }

                    if !found_base {
                        templates_vec.push(BUILTINS.clone());
                    }
                }

                let templates = Arc::new(templates_vec);
                Ok(templates)
            }
        }
    }

    /// Gets all key expiries from the database
    pub async fn get_key_expiries(&self) -> Result<HashMap<Id, Vec<Arc<KeyExpiry>>>, crate::Error> {
        #[derive(sqlx::FromRow)]
        struct KeyExpiryPartial {
            guild_id: String,
            id: String,
            key: String,
            scopes: Vec<String>,
            expires_at: chrono::DateTime<chrono::Utc>,
        }

        let partials: Vec<KeyExpiryPartial> =
            sqlx::query_as("SELECT guild_id, id, key, scopes, expires_at FROM guild_templates_kv WHERE expires_at IS NOT NULL ORDER BY expires_at DESC")
            .fetch_all(&self.state.pool)
            .await?;

        let mut expiries: HashMap<Id, Vec<Arc<KeyExpiry>>> = HashMap::new();

        for partial in partials {
            let guild_id = partial.guild_id.parse()?;

            let expiry = Arc::new(KeyExpiry {
                id: partial.id,
                key: partial.key,
                scopes: partial.scopes,
                expires_at: partial.expires_at,
            });

            let id = Id::GuildId(guild_id);
            if let Some(expiries_vec) = expiries.get_mut(&id) {
                expiries_vec.push(expiry);
            } else {
                expiries.insert(id, vec![expiry]);
            }
        }

        Ok(expiries)
    }

    /// Gets key expiries for a specific tenant
    pub async fn get_key_expiries_for(&self, id: Id) -> Result<ArcVec<KeyExpiry>, crate::Error> {
        match id {
            Id::GuildId(guild_id) => {
                #[derive(sqlx::FromRow)]
                struct KeyExpiryPartial {
                    id: String,
                    key: String,
                    scopes: Vec<String>,
                    expires_at: chrono::DateTime<chrono::Utc>,
                }

                let executions_vec: Vec<KeyExpiryPartial> = sqlx::query_as(
                    "SELECT id, key, scopes, expires_at FROM guild_templates_kv WHERE guild_id = $1 AND expires_at IS NOT NULL ORDER BY expires_at DESC",
                )
                .bind(guild_id.to_string())
                .fetch_all(&self.state.pool)
                .await?;

                let executions_vec = executions_vec
                    .into_iter()
                    .map(|partial| {
                        Arc::new(KeyExpiry {
                            id: partial.id,
                            key: partial.key,
                            scopes: partial.scopes,
                            expires_at: partial.expires_at,
                        })
                    })
                    .collect::<Vec<_>>();

                Ok(executions_vec.into())
            }
        }
    }

    /// Gets all resume keys in the database
    pub async fn get_resume_keys(&self) -> Result<HashMap<Id, Vec<KeyResume>>, crate::Error> {
        #[derive(sqlx::FromRow)]
        struct KeyResumePartial {
            id: String,
            key: String,
            scopes: Vec<String>,
            guild_id: String
        }

        let partials: Vec<KeyResumePartial> =
            sqlx::query_as("SELECT guild_id, id, key, scopes FROM guild_templates_kv WHERE resume = true")
                .fetch_all(&self.state.pool)
                .await?;

        let mut resumes: HashMap<Id, Vec<KeyResume>> = HashMap::new();
        for partial in partials {
            let guild_id = partial.guild_id.parse()?;

            let resume = KeyResume {
                id: partial.id,
                key: partial.key,
                scopes: partial.scopes,
            };
            
            let id = Id::GuildId(guild_id);
            if let Some(resumes_vec) = resumes.get_mut(&id) {
                resumes_vec.push(resume);
            } else {
                resumes.insert(id, vec![resume]);
            }
        }

        Ok(resumes)
    }

    /// Gets resume keys for a specific tenant
    pub async fn get_resume_keys_for(&self, id: Id) -> Result<Vec<KeyResume>, crate::Error> {
        match id {
            Id::GuildId(guild_id) => {
                #[derive(sqlx::FromRow)]
                struct KeyResumePartial {
                    id: String,
                    key: String,
                    scopes: Vec<String>,
                }

                let partials: Vec<KeyResumePartial> =
                    sqlx::query_as("SELECT id, key, scopes FROM guild_templates_kv WHERE resume = true AND guild_id = $1")
                        .bind(guild_id.to_string())
                        .fetch_all(&self.state.pool)
                        .await?;

                let resumes = partials.into_iter().map(|partial| {
                    KeyResume {
                        id: partial.id,
                        key: partial.key,
                        scopes: partial.scopes,
                    }
                }).collect::<Vec<_>>();

                Ok(resumes)
            }
        }
    }

    /// Removes keys with the given ID
    pub async fn remove_key_expiry(&self, id: Id, kv_id: &str) -> Result<(), crate::Error> {
        match id {
            Id::GuildId(guild_id) => {
                sqlx::query("DELETE FROM guild_templates_kv WHERE guild_id = $1 AND id = $2")
                    .bind(guild_id.to_string())
                    .bind(kv_id)
                    .execute(&self.state.pool)
                    .await?;
            }
        }

        Ok(())
    }
}