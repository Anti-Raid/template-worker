use chrono::{DateTime, Utc};
use dapi::{GuildId, Permissions, types::PartialGuild};
use khronos_runtime::utils::khronos_value::KhronosValue;
use serde::{Deserialize, Serialize};
use crate::geese::urlsign;
use crate::worker::workervmmanager::Id;
use crate::master::syscall::{MSyscallContext, MSyscallError, MSyscallHandler, types::guild::{GuildBackup, GuildLockdown}};

/// Backup metadata and the backup blob share one tenant_kv row in this scope
const BACKUP_SCOPE: &str = "builtins.backups.metadata";
/// How long a backup download link stays valid, in seconds
const BACKUP_URL_EXPIRY: u64 = 300;

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MGuildSyscall {
    ListLockdowns { guild_id: GuildId },
    ListBackups { guild_id: GuildId },
    GetBackupUrl { guild_id: GuildId, id: String }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MGuildSyscallRet {
    Lockdowns { lockdowns: Vec<GuildLockdown> },
    Backups { backups: Vec<GuildBackup> },
    BackupUrl { url: String }
}

impl MGuildSyscall {
    pub(super) async fn exec(self, handler: &MSyscallHandler, ctx: MSyscallContext) -> Result<MGuildSyscallRet, MSyscallError> {
        match self {
            Self::ListLockdowns { guild_id } => {
                handler.limit(&ctx, "ListLockdowns")?;
                chk(handler, ctx, guild_id).await?;
                #[derive(sqlx::FromRow)]
                struct Rw { key: String, value: serde_json::Value, created_at: DateTime<Utc> }
                let rows: Vec<Rw> = sqlx::query_as("SELECT key, value, created_at FROM tenant_kv WHERE owner_id = $1 AND owner_type = 'guild' AND scope = $2 ORDER BY created_at DESC LIMIT $3")
                    .bind(guild_id.to_string()).bind("builtins.lockdowns").bind(1000i64)
                    .fetch_all(&handler.pool).await?;
                let mut lockdowns = vec![];
                for r in rows {
                    let v = serde_json::from_value::<KhronosValue>(r.value).ok();
                    lockdowns.push(GuildLockdown { id: r.key, r#type: v.as_ref().and_then(|v| tf(v, "type")), reason: v.as_ref().and_then(|v| tf(v, "reason")), created_at: r.created_at });
                }
                Ok(MGuildSyscallRet::Lockdowns { lockdowns })
            }
            Self::ListBackups { guild_id } => {
                handler.limit(&ctx, "ListBackups")?;
                chk(handler, ctx, guild_id).await?;
                #[derive(sqlx::FromRow)]
                struct Rw { key: String, value: serde_json::Value, size: Option<i64>, created_at: DateTime<Utc>, last_updated_at: DateTime<Utc> }
                let rows: Vec<Rw> = sqlx::query_as("SELECT key, value, octet_length(blob)::bigint AS size, created_at, last_updated_at FROM tenant_kv WHERE owner_id = $1 AND owner_type = 'guild' AND scope = $2 ORDER BY created_at DESC LIMIT $3")
                    .bind(guild_id.to_string()).bind(BACKUP_SCOPE).bind(1000i64)
                    .fetch_all(&handler.pool).await?;
                let backups = rows.into_iter().map(|r| GuildBackup {
                    created_by: serde_json::from_value::<KhronosValue>(r.value).ok().and_then(|v| tf(&v, "createdby")),
                    id: r.key, size: r.size, created_at: r.created_at, last_updated_at: r.last_updated_at,
                }).collect();
                Ok(MGuildSyscallRet::Backups { backups })
            }
            Self::GetBackupUrl { guild_id, id } => {
                handler.limit(&ctx, "GetBackupUrl")?;
                chk(handler, ctx, guild_id).await?;
                let exists: Option<bool> = sqlx::query_scalar("SELECT blob IS NOT NULL FROM tenant_kv WHERE owner_id = $1 AND owner_type = 'guild' AND scope = $2 AND key = $3")
                    .bind(guild_id.to_string()).bind(BACKUP_SCOPE).bind(&id)
                    .fetch_optional(&handler.pool).await?;
                if exists != Some(true) { return Err(MSyscallError::EntityNotFound { reason: "Backup not found" }) }
                let url = urlsign::create_url(Id::Guild(guild_id), &id, BACKUP_SCOPE, BACKUP_URL_EXPIRY)?;
                Ok(MGuildSyscallRet::BackupUrl { url })
            }
        }
    }
}

pub(super) async fn chk(handler: &MSyscallHandler, ctx: MSyscallContext, guild_id: GuildId) -> Result<(), MSyscallError> {
    if ctx.is_secure() { return Ok(()) }
    let user_id = ctx.into_user_id()?;
    if !handler.has_bot_single(guild_id).await? { return Err(MSyscallError::BotNotOnGuild) }
    let Some(member) = handler.guild_member(guild_id, user_id).await? else {
        return Err(MSyscallError::Unauthorized { reason: "You are potentially not a member of this server! If you recently joined the server, you may need to wait up to 5 minutes." });
    };
    let Some(g) = handler.stratum.guild(guild_id).await? else { return Err(MSyscallError::EntityNotFound { reason: "Failed to fetch guild data from stratum" }) };
    let g = serde_json::from_value::<PartialGuild>(g)?;
    if !g.member_permissions(&member).intersects(Permissions::ADMINISTRATOR | Permissions::MANAGE_GUILD) {
        return Err(MSyscallError::Unauthorized { reason: "You need the Manage Server or Administrator permission to view this" });
    }
    Ok(())
}

fn tf(v: &KhronosValue, f: &str) -> Option<String> {
    let x = match v {
        KhronosValue::StrMap(m) => m.get(f)?,
        KhronosValue::Map(m) => m.iter().find_map(|(k, v)| match k { KhronosValue::Text(k) if k == f => Some(v), _ => None })?,
        _ => return None
    };
    if let KhronosValue::Text(s) = x { Some(s.to_string()) } else { None }
}
