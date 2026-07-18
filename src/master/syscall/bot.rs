use std::sync::Arc;

use dapi::types::CreateCommand;
use khronos_runtime::{utils::khronos_value::{CKhronosValue, KhronosValue}};
use serde::{Deserialize, Serialize};
use dapi::UserId;
use crate::{geese::{state::{StateDbFlags, StateExecResult, StateOp}, tenantstate::{ModFlags, TenantState}}, master::syscall::{MSyscallContext, MSyscallError, MSyscallHandler, types::bot::{BotStatus, ShardConn}}, worker::{workerdispatch::SimpleEvent, workervmmanager::Id}};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MBotSyscall {
    /// Returns the commands registered on the bot
    GetBotCommands {},
    /// Returns the bots base config
    GetBotConfig {},
    /// Returns the bots status
    GetBotStatus {},
    /// Dispatch an event to a worker process
    DispatchEvent {
        /// Tenant ID to dispatch the event to
        id: Id,
        /// Name of the event
        name: String,
        /// Data to send
        data: KhronosValue
    },
    /// Dispatch an event (with compressed khronos value) as data to a worker process
    DispatchCEvent {
        /// Tenant ID to dispatch the event to
        id: Id,
        /// Name of the event
        name: String,
        /// Data to send
        data: CKhronosValue
    },
    UserTicket {
        /// Tenant ID for the request
        id: Id
    },
    /// Verify a presigned URL and return the decoded payload
    GetBlobData {
        /// Payload 
        payload: String,
        /// Signature 
        signature: String,
    },
    /// Dispatch an event to a worker process with some safety checks removed
    AdminRelaxedDispatchEvent {
        /// Tenant ID to dispatch the event to
        id: Id,
        /// Name of the event
        name: String,
        /// Data to send
        data: KhronosValue,
        /// Whether or not to allow non-Web event names
        allow_non_web_event_names: bool,
        /// Whether or not to allow self-events
        allow_self_event: bool,
        /// The author ID to mock
        mock_id: Option<String>
    },
    /// Returns the uncached bot status (works in secure contexts only)
    AdminGetUncachedBotStatus {},
    /// Admin API to drop a tenant (works in secure contexts only)
    AdminDropTenant { id: Id },
    /// Admin API to set tenant state moderation flags (ban them etc.) (works in secure contexts only)
    AdminSetTenantStateModFlags { id: Id, modflags: ModFlags },
    /// Admin API to run a set of state ops on a tenant (works in secure contexts only)
    AdminState { id: Id, ops: Vec<StateOp> },
    /// Admin API to fetch tenant state for a tenant (works in secure contexts only)
    AdminFetchTenantState { id: Id }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "op")]
pub enum MBotSyscallRet {
    /// A list of bot commands
    CommandList {
        /// The ID of the user who created the session
        commands: Arc<Vec<CreateCommand>>
    },
    /// Bot config
    BotConfig {
        /// Discord Support Server Link
        support_server_invite: String,
        /// The ID of the AntiRaid bot client
        client_id: UserId,
    },
    /// Bot status
    BotStatus {
        status: BotStatus
    },
    /// Khronos value response
    KhronosValue {
        data: KhronosValue
    },
    /// (Compressed) Khronos value response
    CKhronosValue {
        data: CKhronosValue
    },
    /// State exec response (admin only)
    State {
        res: Vec<StateExecResult>,
        new_tenant_state: Option<TenantState>
    },
    /// Tenant state response (admin only)
    TenantState {
        ts: TenantState
    },
    BlobData {
        data: Vec<u8>,
        filename: String,
    },
    UserTicket {
        payload: String,
        sig: String
    },
    Ack,
}

impl MBotSyscall {
    pub(super) async fn exec(self, handler: &MSyscallHandler, ctx: MSyscallContext) -> Result<MBotSyscallRet, MSyscallError> {
        match self {
            Self::GetBotCommands {  } => {
                Ok(MBotSyscallRet::CommandList { commands: crate::master::register::REGISTER.commands.clone() })
            }
            Self::GetBotConfig {  } => {
                Ok(MBotSyscallRet::BotConfig { 
                    client_id: crate::CONFIG.client_id,
                    support_server_invite: crate::CONFIG.support_server_invite.clone(),
                })
            }
            Self::GetBotStatus {  } => {
                let status = handler.status_cache.try_get_with::<_, crate::Error>((), async move {
                    let raw_stats = handler.stratum.get_status().await?;
                    let uptime = chrono::Utc::now()
                        .signed_duration_since(crate::CONFIG.start_time)
                        .num_seconds()
                        .max(0) as u64;

                    let stats = BotStatus {
                        shard_conns: raw_stats.shards.into_iter().map(|shard| {
                            (shard.shard_id, ShardConn {
                                status: shard.state().as_str_name().to_string(),
                                latency: shard.latency,
                            })
                        }).collect(),
                        total_guilds: raw_stats.guild_count,
                        total_users: raw_stats.user_count,
                        uptime
                    };

                    Ok(stats)
                }).await?;

                Ok(MBotSyscallRet::BotStatus { status })
            }
            Self::DispatchEvent { id, name, data } => {
                if !name.starts_with("Web") {
                    return Err(MSyscallError::InvalidEvent { reason: "Event name must start with Web"});
                }
                let user_id = ctx.into_user_id()?;
                match id {
                    Id::Guild(id) => {
                        // Ensure the bot is in the guild
                        let hb = handler.has_bot_single(id).await?;    
                        if !hb {
                            return Err(MSyscallError::BotNotOnGuild);
                        }
                        // Ensure guild is in server
                    }
                    Id::User(id) => {
                        if user_id != id {
                            return Err(MSyscallError::InvalidEvent { reason: "Cannot send events to users who are not yourself" });
                        }
                    }
                }

                let event = SimpleEvent::new_khronos_value(name, Some(user_id.to_string()), data);

                Ok(MBotSyscallRet::KhronosValue { data: handler.worker_pool.dispatch_event(id, event).await? })
            }
            Self::DispatchCEvent { id, name, data } => {
                if !name.starts_with("Web") {
                    return Err(MSyscallError::InvalidEvent { reason: "Event name must start with Web"});
                }
                let user_id = ctx.into_user_id()?;
                match id {
                    Id::Guild(id) => {
                        // Ensure the bot is in the guild
                        let hb = handler.has_bot(&[id]).await?;    
                        if !hb[0] {
                            return Err(MSyscallError::BotNotOnGuild);
                        }
                    }
                    Id::User(id) => {
                        if user_id != id {
                            return Err(MSyscallError::InvalidEvent { reason: "Cannot send events to users who are not yourself" });
                        }
                    }
                }

                let event = SimpleEvent::new_khronos_value(name, Some(user_id.to_string()), data.0);

                Ok(MBotSyscallRet::CKhronosValue { data: CKhronosValue(handler.worker_pool.dispatch_event(id, event).await?) })
            }
            Self::GetBlobData { payload, signature } => {
                if !ctx.is_anon_getter() && !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }
                // Verify the provided URL, then fetch blob
                let verified = crate::geese::urlsign::verify_url(&payload, &signature).map_err(|e| MSyscallError::Unauthorized { reason: e.message() })?;
                let (data, filename) = handler.statedb.fetch_blob(verified).await?
                .ok_or(MSyscallError::EntityNotFound { reason: "Blob not found" })?;
                Ok(MBotSyscallRet::BlobData { data, filename })
            },
            Self::UserTicket { id } => {
                let user_id = ctx.into_user_id()?;
                handler.limit(&ctx, "UserTicket")?;
                match id {
                    Id::Guild(id) => {
                        // Ensure the bot is in the guild
                        let hb = handler.has_bot(&[id]).await?;    
                        if !hb[0] {
                            return Err(MSyscallError::BotNotOnGuild);
                        }
                    }
                    Id::User(id) => {
                        if user_id != id {
                            return Err(MSyscallError::InvalidEvent { reason: "Cannot create tickets to users who are not yourself" });
                        }
                    }
                }
                let (payload, sig) = crate::geese::userticket::create_userticket(id, user_id)?;
                Ok(MBotSyscallRet::UserTicket { payload, sig }) 
            }
            Self::AdminRelaxedDispatchEvent { id, name, data, allow_non_web_event_names, allow_self_event, mock_id } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }

                if !allow_non_web_event_names && !name.starts_with("Web") {
                    return Err(MSyscallError::InvalidEvent { reason: "Event name must start with Web"});
                }

                let user_id = match mock_id {
                    Some(id) => id.parse()?,
                    None => ctx.into_user_id()?
                };
                
                match id {
                    Id::Guild(id) => {
                        // Ensure the bot is in the guild
                        let hb = handler.has_bot(&[id]).await?;    
                        if !hb[0] {
                            return Err(MSyscallError::BotNotOnGuild);
                        }
                        // Ensure guild is in server
                    }
                    Id::User(id) => {
                        if !allow_self_event && user_id != id {
                            return Err(MSyscallError::InvalidEvent { reason: "Cannot send events to users who are not yourself" });
                        }
                    }
                }

                let event = SimpleEvent::new_khronos_value(name, Some(user_id.to_string()), data);

                Ok(MBotSyscallRet::KhronosValue { data: handler.worker_pool.dispatch_event(id, event).await? })
            }
            Self::AdminGetUncachedBotStatus {  } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }

                let raw_stats = handler.stratum.get_status().await?;
                let uptime = chrono::Utc::now()
                    .signed_duration_since(crate::CONFIG.start_time)
                    .num_seconds()
                    .max(0) as u64;

                let status = BotStatus {
                    shard_conns: raw_stats.shards.into_iter().map(|shard| {
                        (shard.shard_id, ShardConn {
                            status: shard.state().as_str_name().to_string(),
                            latency: shard.latency,
                        })
                    }).collect(),
                    total_guilds: raw_stats.guild_count,
                    total_users: raw_stats.user_count,
                    uptime
                };

                Ok(MBotSyscallRet::BotStatus { status })
            }
            Self::AdminDropTenant { id } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }

                handler.worker_pool.drop_tenant(id).await?;
                Ok(MBotSyscallRet::Ack)
            }
            Self::AdminSetTenantStateModFlags { id, modflags } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }
                let mut tx = handler.pool.begin().await?;
                sqlx::query("INSERT INTO tenant_state (owner_id, owner_type, modflags) VALUES ($1, $2, $3) ON CONFLICT (owner_id, owner_type) DO UPDATE SET modflags = EXCLUDED.modflags")
                    .bind(id.tenant_id())
                    .bind(id.tenant_type())
                    .bind(modflags.bits() as i32)
                    .execute(&mut *tx)
                    .await?;

                // Refresh tenant state now
                let Some(ts) = handler.tsdb.get_tenant_state_for(&mut tx, id).await? else {
                    return Err("failed to find tenant state after update".into())
                };

                tx.commit().await?;
                
                handler.worker_pool.update_tenant_state(id, ts).await?;
                Ok(MBotSyscallRet::Ack)
            }
            Self::AdminState { id, ops } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }

                let res = handler.statedb.do_op(id, ops, StateDbFlags::ADMIN).await?;

                // inform worker of new tenant state if we have a new tenant state
                if let Some(ref new_ts) = res.new_tenant_state {
                    handler.worker_pool.update_tenant_state(id, new_ts.clone()).await?;
                }

                Ok(MBotSyscallRet::State { res: res.results, new_tenant_state: res.new_tenant_state })
            }
            Self::AdminFetchTenantState { id } => {
                if !ctx.is_secure() {
                    return Err(MSyscallError::ContextInsecure);
                }

                let mut tx = handler.pool.begin().await?;
                let ts = handler.tsdb.get_tenant_state_for(&mut tx, id).await?;
                tx.commit().await?;

                let Some(ts) = ts else {
                    return Err(MSyscallError::EntityNotFound { reason: "tenant state not found" });
                };

                Ok(MBotSyscallRet::TenantState { ts })
            }
        }
    }
}
