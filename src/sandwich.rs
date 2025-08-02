use crate::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;

/// Returns the current user from Sandwich
pub async fn current_user(
    reqwest_client: &reqwest::Client,
) -> Result<serenity::all::CurrentUser, Error> {
    let url = format!(
        "{}/antiraid/api/current-user",
        crate::CONFIG.meta.sandwich_http_api
    );

    #[derive(Serialize, Deserialize)]
    struct Resp {
        ok: bool,
        data: Option<serenity::all::CurrentUser>,
        error: Option<String>,
    }

    let resp = reqwest_client.get(&url).send().await?.json::<Resp>().await?;

    if resp.ok {
        resp.data.ok_or_else(|| "No current user found".into())
    } else {
        Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()).into())
    }
}

pub async fn has_guilds(
    reqwest_client: &reqwest::Client,
    guilds: Vec<serenity::all::GuildId>,
) -> Result<Vec<u8>, Error> {
    #[derive(Serialize, Deserialize)]
    struct Resp {
        ok: bool,
        data: Option<Vec<u8>>,
        error: Option<String>,
    }

    let url = format!(
        "{}/antiraid/api/bulk-has-guild",
        crate::CONFIG.meta.sandwich_http_api
    );

    let resp = reqwest_client
        .post(&url)
        .json(&guilds)
        .send()
        .await?
        .json::<Resp>()
        .await?;

    if resp.ok {
        let Some(has_guild_id) = resp.data else {
            return Err("Could not fetch guilds that are known to be in bots cache".into());
        };

        return Ok(has_guild_id);
    } else {
        return Err(
            resp.error.unwrap_or_else(|| "Unknown error".to_string()).into(),
        );
    }    
}

/// Fetches a guild while handling all the pesky errors serenity normally has
/// with caching
pub async fn guild(
    http: &serenity::http::Http,
    reqwest_client: &reqwest::Client,
    guild_id: serenity::model::id::GuildId,
) -> Result<Value, Error> {    
    // Check sandwich, it may be there
    let url = format!(
        "{}/antiraid/api/state?col=guilds&id={}",
        crate::CONFIG.meta.sandwich_http_api,
        guild_id
    );

    #[derive(serde::Serialize, serde::Deserialize, Debug)]
    struct Resp {
        ok: bool,
        data: Option<Value>,
        error: Option<String>,
    }

    let resp = reqwest_client.get(&url).send().await?.json::<Resp>().await;

    if let Ok(resp) = resp {
        if resp.ok {
            let Some(guild) = resp.data else {
                return Err("Guild not found".into());
            };

            return Ok(guild);
        } else {
            log::warn!(
                "Sandwich proxy returned error [get guild]: {:?}",
                resp.error
            );
        }
    } else {
        log::warn!(
            "Sandwich proxy returned invalid resp [get guild]: {:?}",
            resp
        );
    }

    // Last resort: make the http call
    let res = http.get_guild_with_counts(guild_id).await?;

    // Save to sandwich
    /*let url = format!(
        "{}/antiraid/api/state?col=guilds&id={}",
        crate::CONFIG.meta.sandwich_http_api,
        guild_id
    );

    let resp = reqwest_client.post(&url).json(&res).send().await?;

    if !resp.status().is_success() {
        log::warn!(
            "Failed to update sandwich proxy with guild data: {:?}",
            resp.text().await
        );
    }*/

    Ok(res)
}

/// Returns a member in a guild using sandwich proxy
/// If the member is not found in the sandwich proxy, it will fetch it from the HTTP
/// API and update the sandwich proxy with the member data  
pub async fn member_in_guild(
    http: &serenity::http::Http,
    reqwest_client: &reqwest::Client,
    guild_id: serenity::model::id::GuildId,
    user_id: serenity::model::id::UserId,
) -> Result<Option<Value>, Error> {
    let url = format!(
        "{}/antiraid/api/state?col=members&id={}&guild_id={}",
        crate::CONFIG.meta.sandwich_http_api,
        user_id,
        guild_id
    );

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Resp {
        ok: bool,
        data: Option<Value>,
        error: Option<String>,
    }

    let resp = reqwest_client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?
        .json::<Resp>()
        .await;

    match resp {
        Ok(resp) => {
            if resp.ok {
                let Some(member) = resp.data else {
                    return Ok(None);
                };

                return Ok(Some(member));
            } else {
                log::warn!(
                    "Sandwich proxy returned error [get member]: {:?}",
                    resp.error
                );
            }
        }
        Err(e) => {
            log::warn!("Failed to fetch member (http): {:?}", e);
        }
    }

    let member = match http.get_member(guild_id, user_id).await {
        Ok(mem) => mem,
        Err(e) => match e {
            serenity::Error::Http(e) => match e {
                serenity::all::HttpError::UnsuccessfulRequest(er) => {
                    if er.status_code == reqwest::StatusCode::NOT_FOUND {
                        return Ok(None);
                    } else {
                        return Err(
                            format!("Failed to fetch member (http, non-404): {:?}", er).into()
                        );
                    }
                }
                _ => {
                    return Err(format!("Failed to fetch member (http): {:?}", e).into());
                }
            },
            _ => {
                return Err(format!("Failed to fetch member: {:?}", e).into());
            }
        },
    };

    // Update sandwich with a POST
    /*let resp = reqwest_client.post(&url).json(&member).send().await?;

    if !resp.status().is_success() {
        log::warn!(
            "Failed to update sandwich proxy with member data: {:?}",
            resp.text().await
        );
    }*/

    Ok(Some(member))
}

/// Faster version of serenity guild_roles that also takes into account the sandwich proxy layer
pub async fn guild_roles(
    http: &serenity::http::Http,
    reqwest_client: &reqwest::Client,
    guild_id: serenity::model::id::GuildId,
) -> Result<Value, Error> {
    let url = format!(
        "{}/antiraid/api/state?col=guild_roles&id={}",
        crate::CONFIG.meta.sandwich_http_api,
        guild_id
    );

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Resp {
        ok: bool,
        data: Option<Value>,
        error: Option<String>,
    }

    let resp = reqwest_client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?
        .json::<Resp>()
        .await;

    match resp {
        Ok(resp) => {
            if resp.ok {
                let Some(roles) = resp.data else {
                    return Err("No roles found".into());
                };

                return Ok(roles);
            } else {
                log::warn!(
                    "Sandwich proxy returned error [get guild roles]: {:?}",
                    resp.error
                );
            }
        }
        Err(e) => {
            log::warn!("Failed to fetch member (http): {:?}", e);
        }
    }

    // Last resort, fetch from http and then update sandwich as well
    let roles = match http.get_guild_roles(guild_id).await {
        Ok(mem) => mem,
        Err(e) => match e {
            serenity::Error::Http(e) => match e {
                serenity::all::HttpError::UnsuccessfulRequest(er) => {
                    if er.status_code == reqwest::StatusCode::NOT_FOUND {
                        return Err("No channels found".into());
                    } else {
                        return Err(
                            format!("Failed to fetch roles (http, non-404): {:?}", er).into()
                        );
                    }
                }
                _ => {
                    return Err(format!("Failed to fetch roles (http): {:?}", e).into());
                }
            },
            _ => {
                return Err(format!("Failed to fetch roles: {:?}", e).into());
            }
        },
    };

    // Update sandwich with a POST
    /*let resp = reqwest_client.post(&url).json(&roles).send().await?;

    if !resp.status().is_success() {
        log::warn!(
            "Failed to update sandwich proxy with channel data: {:?}",
            resp.text().await
        );
    }*/

    Ok(roles)
}

/// Faster version of serenity guild_channels that also takes into account the sandwich proxy layer
pub async fn guild_channels(
    http: &serenity::http::Http,
    reqwest_client: &reqwest::Client,
    guild_id: serenity::model::id::GuildId,
) -> Result<Value, Error> {
    let url = format!(
        "{}/antiraid/api/state?col=guild_channels&id={}",
        crate::CONFIG.meta.sandwich_http_api,
        guild_id
    );

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Resp {
        ok: bool,
        data: Option<Value>,
        error: Option<String>,
    }

    let resp = reqwest_client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?
        .json::<Resp>()
        .await;

    match resp {
        Ok(resp) => {
            if resp.ok {
                let Some(channels) = resp.data else {
                    return Err("No channels found".into());
                };

                return Ok(channels);
            } else {
                log::warn!(
                    "Sandwich proxy returned error [get guild channels]: {:?}",
                    resp.error
                );
            }
        }
        Err(e) => {
            log::warn!("Failed to fetch member (http): {:?}", e);
        }
    }

    // Last resort, fetch from http and then update sandwich as well
    let channels = match http.get_channels(guild_id).await {
        Ok(mem) => mem,
        Err(e) => match e {
            serenity::Error::Http(e) => match e {
                serenity::all::HttpError::UnsuccessfulRequest(er) => {
                    if er.status_code == reqwest::StatusCode::NOT_FOUND {
                        return Err("No channels found".into());
                    } else {
                        return Err(
                            format!("Failed to fetch channels (http, non-404): {:?}", er).into(),
                        );
                    }
                }
                _ => {
                    return Err(format!("Failed to fetch channels (http): {:?}", e).into());
                }
            },
            _ => {
                return Err(format!("Failed to fetch channels: {:?}", e).into());
            }
        },
    };

    // Update sandwich with a POST
    /*let resp = reqwest_client.post(&url).json(&channels).send().await?;

    if !resp.status().is_success() {
        log::warn!(
            "Failed to update sandwich proxy with channel data: {:?}",
            resp.text().await
        );
    }*/

    Ok(channels)
}

pub async fn channel(
    http: &serenity::http::Http,
    reqwest_client: &reqwest::Client,
    guild_id: Option<serenity::model::id::GuildId>,
    channel_id: serenity::model::id::GenericChannelId,
) -> Result<Option<Value>, Error> {
    let url = match guild_id {
        Some(guild_id) => format!(
            "{}/antiraid/api/state?col=channels&id={}&guild_id={}",
            crate::CONFIG.meta.sandwich_http_api,
            channel_id,
            guild_id
        ),
        None => format!(
            "{}/antiraid/api/state?col=channels&id={}",
            crate::CONFIG.meta.sandwich_http_api,
            channel_id
        ),
    };

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Resp {
        ok: bool,
        data: Option<Value>,
        error: Option<String>,
    }

    let resp = reqwest_client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await?;

    let status = resp.status();

    let json = resp.json::<Resp>().await?;

    if json.ok {
        return Ok(json.data);
    } else {
        log::warn!(
            "Sandwich proxy returned error [get channel]: {:?}, status: {:?}",
            json.error,
            status
        );
    }

    // Last resort, fetch from http and then update sandwich as well
    let channel = match http.get_channel(channel_id).await {
        Ok(channel) => channel,
        Err(e) => match e {
            serenity::Error::Http(e) => match e {
                serenity::all::HttpError::UnsuccessfulRequest(er) => {
                    if er.status_code == reqwest::StatusCode::NOT_FOUND {
                        return Ok(None);
                    } else {
                        return Err(
                            format!("Failed to fetch channels (http, non-404): {:?}", er).into(),
                        );
                    }
                }
                _ => {
                    return Err(format!("Failed to fetch channels (http): {:?}", e).into());
                }
            },
            _ => {
                return Err(format!("Failed to fetch channels: {:?}", e).into());
            }
        },
    };

    // Update sandwich with a POST
    /*let resp = reqwest_client
        .post(&url)
        .timeout(std::time::Duration::from_secs(10))
        .json(&channel)
        .send()
        .await?;

    if !resp.status().is_success() {
        log::warn!(
            "Failed to update sandwich proxy with channel data: {:?}",
            resp.text().await
        );
    }*/

    Ok(Some(channel))
}

pub async fn get_status(client: &reqwest::Client) -> Result<GetStatusResponse, Error> {
    let res = client
        .get(format!(
            "{}/api/status",
            crate::CONFIG.meta.sandwich_http_api
        ))
        .send()
        .await?
        .error_for_status()?
        .json::<Resp<StatusEndpointResponse>>()
        .await?;

    if !res.ok {
        return Err("Sandwich API returned not ok".into());
    }

    let Some(data) = res.data else {
        return Err("No data in response".into());
    };

    // Parse out the shard connections
    let mut shards = HashMap::new();

    for manager in data.managers.iter() {
        if manager.display_name != *"Anti Raid" {
            continue; // Not for us
        }

        for v in manager.shard_groups.iter() {
            for shard in v.shards.iter() {
                let shard_id = shard[0];
                let status = shard[1];
                let latency = shard[2];
                let guilds = shard[3];
                let uptime = shard[4];
                let total_uptime = shard[5];

                shards.insert(
                    shard_id,
                    ShardConn {
                        status: match status {
                            0 => "Idle".to_string(),
                            1 => "Connecting".to_string(),
                            2 => "Connected".to_string(),
                            3 => "MarkedForClosure".to_string(),
                            4 => "Closing".to_string(),
                            5 => "Closed".to_string(),
                            6 => "Erroring".to_string(),
                            _ => "Unknown".to_string(),
                        },
                        real_latency: latency,
                        guilds,
                        uptime,
                        total_uptime,
                    },
                );
            }
        }
    }

    Ok(GetStatusResponse {
        resp: data,
        shard_conns: shards,
    })
}

#[allow(dead_code)]
pub struct ShardConn {
    pub status: String,
    pub real_latency: i64,
    pub guilds: i64,
    pub uptime: i64,
    pub total_uptime: i64,
}

#[allow(dead_code)]
pub struct GetStatusResponse {
    pub resp: StatusEndpointResponse,
    pub shard_conns: HashMap<i64, ShardConn>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ShardGroupStatus {
    Idle,
    Connecting,
    Connected,
    MarkedForClosure,
    Closing,
    Closed,
    Erroring,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusEndpointResponse {
    pub uptime: i64,
    pub managers: Vec<StatusEndpointManager>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusEndpointManager {
    pub display_name: String,
    pub shard_groups: Vec<StatusEndpointShardGroup>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StatusEndpointShardGroup {
    #[serde(rename = "id")]
    pub shard_group_id: i32,
    pub shards: Vec<[i64; 6]>, // // ShardID, Status, Latency (in milliseconds), Guilds, Uptime (in seconds), Total Uptime (in seconds)
    pub status: ShardGroupStatus,
    pub uptime: i64,
}

#[derive(Serialize, Deserialize)]
pub struct Resp<T> {
    pub ok: bool,
    pub data: Option<T>,
}
