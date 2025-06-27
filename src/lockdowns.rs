use std::collections::HashSet;
use std::sync::Arc;

use lockdowns::{
    from_lockdown_mode_string, CreateLockdown, GuildLockdownSettings, Lockdown, LockdownDataStore,
};
use sqlx::Row;

#[derive(Clone)]
pub struct LockdownData {
    pub cache: Arc<serenity::all::Cache>,
    pub http: Arc<serenity::all::Http>,
    pub pool: sqlx::PgPool,
    pub reqwest: reqwest::Client,
}

impl LockdownData {
    pub fn new(
        cache: Arc<serenity::all::Cache>,
        http: Arc<serenity::all::Http>,
        pool: sqlx::PgPool,
        reqwest: reqwest::Client,
    ) -> Self {
        Self {
            cache,
            http,
            pool,
            reqwest,
        }
    }
}

#[derive(sqlx::FromRow)]
struct LockdownRow {
    id: uuid::Uuid,
    r#type: String,
    data: serde_json::Value,
    reason: String,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl LockdownDataStore for LockdownData {
    async fn get_guild_lockdown_settings(
        &self,
        guild_id: serenity::all::GuildId,
    ) -> Result<lockdowns::GuildLockdownSettings, lockdowns::Error> {
        match sqlx::query(
            "SELECT member_roles, require_correct_layout FROM lockdown__guilds WHERE guild_id = $1",
        )
        .bind(guild_id.to_string())
        .fetch_optional(&self.pool)
        .await?
        {
            Some(settings) => {
                let member_roles = {
                    let member_roles_vec = settings.try_get::<Vec<String>, _>("member_roles")?;

                    let mut member_roles = HashSet::with_capacity(member_roles_vec.len());
                    for role in member_roles_vec {
                        if role.is_empty() {
                            continue;
                        }

                        member_roles.insert(
                            role.parse()
                                .map_err(|_| format!("Failed to parse role ID: {}", role,))?,
                        );
                    }

                    member_roles
                };

                let settings = GuildLockdownSettings {
                    member_roles,
                    require_correct_layout: settings.try_get("require_correct_layout")?,
                };

                Ok(settings)
            }
            None => Ok(GuildLockdownSettings::default()),
        }
    }

    async fn guild(
        &self,
        guild_id: serenity::all::GuildId,
    ) -> Result<serenity::all::PartialGuild, lockdowns::Error> {
        crate::sandwich::guild(&self.cache, &self.http, &self.reqwest, guild_id).await
    }

    async fn guild_channels(
        &self,
        guild_id: serenity::all::GuildId,
    ) -> Result<Vec<serenity::all::GuildChannel>, lockdowns::Error> {
        crate::sandwich::guild_channels(&self.cache, &self.http, &self.reqwest, guild_id).await
    }

    fn cache(&self) -> Option<&serenity::all::Cache> {
        Some(self.cache.as_ref())
    }

    fn http(&self) -> &serenity::all::Http {
        &self.http
    }

    async fn get_lockdowns(
        &self,
        guild_id: serenity::all::GuildId,
    ) -> Result<Vec<Lockdown>, lockdowns::Error> {
        let data: Vec<LockdownRow> = sqlx::query_as(
            "SELECT id, type, data, reason, created_at FROM lockdown__guild_lockdowns WHERE guild_id = $1",
        )
        .bind(guild_id.to_string())
        .fetch_all(&self.pool)
        .await?;

        let mut lockdowns = Vec::new();

        for row in data {
            let id = row.id;
            let r#type = row.r#type;
            let data = row.data;
            let reason = row.reason;
            let created_at = row.created_at;

            let lockdown_mode = from_lockdown_mode_string(&r#type)?;

            let lockdown = Lockdown {
                id,
                r#type: lockdown_mode,
                data,
                reason,
                created_at,
            };

            lockdowns.push(lockdown);
        }

        Ok(lockdowns)
    }

    async fn insert_lockdown(
        &self,
        guild_id: serenity::all::GuildId,
        lockdown: CreateLockdown,
    ) -> Result<Lockdown, lockdowns::Error> {
        let id = sqlx::query(
            "INSERT INTO lockdown__guild_lockdowns (guild_id, type, data, reason) VALUES ($1, $2, $3, $4) RETURNING id, created_at",
        )
        .bind(guild_id.to_string())
        .bind(lockdown.r#type.string_form())
        .bind(&lockdown.data)
        .bind(&lockdown.reason)
        .fetch_one(&self.pool)
        .await?;

        Ok(Lockdown {
            id: id.try_get("id")?,
            r#type: lockdown.r#type,
            data: lockdown.data,
            reason: lockdown.reason,
            created_at: id.try_get("created_at")?,
        })
    }

    async fn remove_lockdown(
        &self,
        guild_id: serenity::all::GuildId,
        id: uuid::Uuid,
    ) -> Result<(), lockdowns::Error> {
        sqlx::query("DELETE FROM lockdown__guild_lockdowns WHERE guild_id = $1 AND id = $2")
            .bind(guild_id.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}
