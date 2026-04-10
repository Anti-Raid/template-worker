use serde::Serialize;
use crate::master::syscall::{MSyscallContext, MSyscallError, MSyscallHandler, types::gkv::PartialGlobalKv};

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum MGkvSyscall {
    FindGlobalKvs {
        /// Scope to use for filtering
        scope: String,
        /// The query to filter keys by. Accepts SQL ILIKE syntax'd queries (so % matches >= 1 char, _ matches 1 char and %% will find all keys)
        query: String,
    },
    GetGlobalKv {
        /// Scope of the global kv
        scope: String, 
        /// Key of the global kv
        key: String, 
        /// Version of the global kv
        version: i32
    }
}

#[derive(Serialize)]
#[serde(tag = "op")]
pub enum MGkvSyscallRet {
    GlobalKvList {
        gkvs: Vec<PartialGlobalKv>
    },
    GlobalKv {
        gkv: PartialGlobalKv
    }
}

impl MGkvSyscall {
    pub(super) async fn exec(self, handler: &MSyscallHandler, ctx: MSyscallContext) -> Result<MGkvSyscallRet, MSyscallError> {
        match self {
            Self::FindGlobalKvs { scope, query } => {
                let gkvs= if query == "%%" {
                    sqlx::query_as(
                        "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved'"
                    )
                    .bind(scope)
                    .fetch_all(&handler.pool)
                    .await?
                } else {
                    sqlx::query_as(
                        "SELECT key, version, owner_id, owner_type, short, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE scope = $1 AND review_state = 'approved' AND key LIKE $2"
                    )
                    .bind(scope)
                    .bind(query)
                    .fetch_all(&handler.pool)
                    .await?
                };

                Ok(MGkvSyscallRet::GlobalKvList { gkvs })
            }
            Self::GetGlobalKv { scope, key, version } => {
                let item: Option<PartialGlobalKv> = sqlx::query_as(
                    "SELECT key, version, owner_id, owner_type, short, long, public_metadata, public_data, scope, created_at, last_updated_at, price, review_state FROM global_kv WHERE review_state = 'approved' AND key = $1 AND version = $2 AND scope = $3",
                )
                .bind(&key)
                .bind(version)
                .bind(&scope)
                .fetch_optional(&self.pool)
                .await?;

                let Some(mut gkv) = item else {
                    return Err(MSyscallError::EntityNotFound { reason: "Global kv entry with this scope/key/version pair was not found" });
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

                Ok(MGkvSyscallRet::GlobalKv { gkv })
            }
        }
    }
}
