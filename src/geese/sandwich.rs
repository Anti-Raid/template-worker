use crate::Error;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};
use serenity::all::ResultJson;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Clone)]
pub struct Sandwich {
    pub reqwest_client: reqwest::Client,
    pub http: Arc<serenity::http::Http>
}

impl Sandwich {
    pub fn new(reqwest_client: reqwest::Client, http: Arc<serenity::http::Http>) -> Self {
        Self { reqwest_client, http }
    }

    async fn get<T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
    ) -> Result<T, Error> {
        let url = format!(
            "{}/{}",
            crate::CONFIG.meta.sandwich_http_api, endpoint
        );

        let mut attempts = 0;
        loop {
            attempts += 1;
            if attempts > 10 {
                return Err("Exceeded maximum retry attempts to Sandwich proxy".into());
            }

            let resp = self.reqwest_client.get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send().await?;

            if resp.status().is_success() {
                let json = resp.json::<T>().await?;
                return Ok(json);
            }

            if resp.headers().get("Retry-After").is_some() {
                // Wait and retry
                let wait_duration = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1);

                log::warn!(
                    "Rate limited by Sandwich proxy, retrying after {} seconds",
                    wait_duration
                );
                tokio::time::sleep(std::time::Duration::from_secs(wait_duration)).await;
                continue;
            } else {
                let status = resp.status();
                let url = resp.url().clone();
                let resp = resp.text().await?;
                return Err(format!(
                    "Failed to request Sandwich proxy (status: {}): {} [url: {url}]",
                    status, resp
                ).into());
            }
        }
    }
    
    async fn post<R: Serialize, T: for<'de> Deserialize<'de>>(
        &self,
        endpoint: &str,
        body: &R,
    ) -> Result<T, Error> {
        let url = format!(
            "{}/{}",
            crate::CONFIG.meta.sandwich_http_api, endpoint
        );

        let mut attempts = 0;
        loop {
            attempts += 1;
            if attempts > 10 {
                return Err("Exceeded maximum retry attempts to Sandwich proxy".into());
            }

            let resp = self.reqwest_client.post(&url)
            .timeout(std::time::Duration::from_secs(5))
            .json(body).send().await?;

            if resp.status().is_success() {
                let json = resp.json::<T>().await?;
                return Ok(json);
            }

            if resp.headers().get("Retry-After").is_some() {
                // Wait and retry
                let wait_duration = resp
                    .headers()
                    .get("Retry-After")
                    .and_then(|h| h.to_str().ok())
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(1);

                log::warn!(
                    "Rate limited by Sandwich proxy, retrying after {} seconds",
                    wait_duration
                );
                tokio::time::sleep(std::time::Duration::from_secs(wait_duration)).await;
                continue;
            } else {
                let status = resp.status();
                let url = resp.url().clone();
                let resp = resp.text().await?;
                return Err(format!(
                    "Failed to request Sandwich proxy (status: {}): {} [url: {url}]",
                    status, resp
                ).into());
            }
        }
    }

    /// Helper method to extract value or return None from a serenity http response
    fn extract_value(val: ResultJson) -> Result<Option<Value>, Error> {
        let e = match val {
            Ok(v) => return Ok(Some(v)),
            Err(e) => e,
        };
        match e {
            serenity::Error::Http(e) => match e {
                serenity::all::HttpError::UnsuccessfulRequest(er) => {
                    if er.status_code == reqwest::StatusCode::NOT_FOUND {
                        return Ok(None);
                    } else {
                        return Err(
                            format!("Failed to fetch (http, non-404): {:?}", er).into()
                        );
                    }
                }
                _ => {
                    return Err(format!("Failed to fetch (http): {:?}", e).into());
                }
            },
            _ => {
                return Err(format!("Failed to fetch: {:?}", e).into());
            }
        }
    }

    fn extract_state_resp(resp: Result<StateResp, reqwest::Error>) -> Result<Option<Value>, Error> {
        match resp {
            Ok(state_resp) => state_resp.into_value(),
            Err(e) => Err(format!("Failed to fetch state from Sandwich proxy: {:?}", e).into()),
        }
    }

    /// Returns the current user from Sandwich
    pub async fn current_user(&self) -> Result<serenity::all::CurrentUser, Error> {
        #[derive(Serialize, Deserialize)]
        struct Resp {
            ok: bool,
            data: Option<serenity::all::CurrentUser>,
            error: Option<String>,
        }

        let resp = self.get::<Resp>("antiraid/api/current-user").await?;

        if resp.ok {
            resp.data.ok_or_else(|| "No current user found".into())
        } else {
            Err(resp.error.unwrap_or_else(|| "Unknown error".to_string()).into())
        }
    }

    /// Returns the number of shards from Sandwich
    pub async fn get_shard_count(&self) -> Result<usize, Error> {
        #[derive(Serialize, Deserialize)]
        struct Resp {
            shards: usize,
        }

        let resp = self.get::<Resp>("antiraid/api/gateway/bot").await?;

        if resp.shards == 0 {
            Err("Sandwich returned 0 shards".into())
        } else {
            Ok(resp.shards)
        }
    }

    pub async fn has_guilds(
        &self,
        guilds: &[serenity::all::GuildId],
    ) -> Result<Vec<u8>, Error> {
        #[derive(Serialize, Deserialize)]
        struct Resp {
            ok: bool,
            data: Option<Vec<u8>>,
            error: Option<String>,
        }

        let resp = self.post::<_, Resp>(
            "antiraid/api/bulk-has-guild",
            &guilds,
        )
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
        &self,
        guild_id: serenity::model::id::GuildId,
    ) -> Result<Value, Error> {    
    // Check sandwich, it may be there
    let url = format!(
        "{}/antiraid/api/state?col=guilds&id={}",
        crate::CONFIG.meta.sandwich_http_api,
        guild_id
    );

        let resp = self.reqwest_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .json::<StateResp>()
            .await;

        match Sandwich::extract_state_resp(resp) {
            Ok(Some(guild)) => return Ok(guild),
            Ok(None) => {} // Not found, continue
            Err(e) => {
                log::warn!("Failed to fetch guild from Sandwich proxy: {:?}", e);
            }
        }

        // Last resort: make the http call
        let res = self.http.get_guild_with_counts(guild_id).await?;

        Ok(res)
    }

    /// Returns a member in a guild using sandwich proxy
    /// If the member is not found in the sandwich proxy, it will fetch it from the HTTP
    /// API and update the sandwich proxy with the member data  
    pub async fn member_in_guild(
        &self,
        guild_id: serenity::model::id::GuildId,
        user_id: serenity::model::id::UserId,
    ) -> Result<Option<Value>, Error> {
        let url = format!(
            "{}/antiraid/api/state?col=members&id={}&guild_id={}",
            crate::CONFIG.meta.sandwich_http_api,
            user_id,
            guild_id
        );

        let resp = self.reqwest_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .json::<StateResp>()
            .await;

        match Sandwich::extract_state_resp(resp) {
            Ok(Some(member)) => return Ok(Some(member)),
            Ok(None) => return Ok(None), // Not found
            Err(e) => {
                log::warn!("Failed to fetch member from Sandwich proxy: {:?}", e);
            }
        }

        let member_resp = self.http.get_member(guild_id, user_id).await;
        Sandwich::extract_value(member_resp)
    }

    /// Faster version of serenity guild_roles that also takes into account the sandwich proxy layer
    pub async fn guild_roles(
        &self,
        guild_id: serenity::model::id::GuildId,
    ) -> Result<Value, Error> {
        let url = format!(
            "{}/antiraid/api/state?col=guild_roles&id={}",
            crate::CONFIG.meta.sandwich_http_api,
            guild_id
        );

        let resp = self.reqwest_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .json::<StateResp>()
            .await;

        match Sandwich::extract_state_resp(resp) {
            Ok(Some(roles)) => return Ok(roles),
            Ok(None) => {} // Not found, continue
            Err(e) => {
                log::warn!("Failed to fetch guild roles from Sandwich proxy: {:?}", e);
            }
        }

        // Last resort, fetch from http 
        let roles = self.http.get_guild_roles(guild_id).await;
        match Sandwich::extract_value(roles) {
            Ok(Some(roles)) => return Ok(roles),
            Ok(None) => return Err("No roles found".into()),
            Err(e) => {
                return Err(format!("Failed to fetch roles: {:?}", e).into());
            }
        }
    }

    /// Faster version of serenity guild_channels that also takes into account the sandwich proxy layer
    pub async fn guild_channels(
        &self,
        guild_id: serenity::model::id::GuildId,
    ) -> Result<Value, Error> {
        let url = format!(
            "{}/antiraid/api/state?col=guild_channels&id={}",
            crate::CONFIG.meta.sandwich_http_api,
            guild_id
        );

        let resp = self.reqwest_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .json::<StateResp>()
            .await;

        match Sandwich::extract_state_resp(resp) {
            Ok(Some(roles)) => return Ok(roles),
            Ok(None) => {} // Not found, continue
            Err(e) => {
                log::warn!("Failed to fetch guild roles from Sandwich proxy: {:?}", e);
            }
        }

        // Last resort, fetch from http 
        let channels = self.http.get_channels(guild_id).await;
        match Sandwich::extract_value(channels) {
            Ok(Some(channels)) => return Ok(channels),
            Ok(None) => return Err("No channels found".into()),
            Err(e) => {
                return Err(format!("Failed to fetch channels: {:?}", e).into());
            }
        }
    }

    pub async fn channel(
        &self,
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

        let resp = self.reqwest_client
            .get(&url)
            .timeout(std::time::Duration::from_secs(5))
            .send()
            .await?
            .json::<StateResp>()
            .await;

        match Sandwich::extract_state_resp(resp) {
            Ok(Some(channel)) => return Ok(Some(channel)),
            Ok(None) => {} // Not found, continue
            Err(e) => {
                log::warn!("Failed to fetch channel from Sandwich proxy: {:?}", e);
            }
        }

        // Last resort, fetch from http and then update sandwich as well
        let channel = self.http.get_channel(channel_id).await;
        match Sandwich::extract_value(channel) {
            Ok(v) => return Ok(v),
            Err(e) => {
                return Err(format!("Failed to fetch channel: {:?}", e).into());
            }
        }
    }

    pub async fn get_status(&self) -> Result<GetStatusResponse, Error> {
        let res = self.get::<Resp<StatusEndpointResponse>>("api/status").await?;
        if !res.ok {
            return Err("Sandwich API returned not ok".into());
        }

        let Some(data) = res.data else {
            return Err("No data in response".into());
        };

        let mut user_count = 0;
        let mut total_members = 0;

        // Parse out the shard connections
        let mut shards = HashMap::new();

        for manager in data.managers.iter() {
            if manager.display_name != *"Anti Raid" {
                continue; // Not for us
            }

            user_count = manager.user_count;
            total_members = manager.member_count;

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
                                3 => "Ready".to_string(),
                                4 => "Reconnecting".to_string(),
                                5 => "Closing".to_string(),
                                6 => "Closed".to_string(),
                                7 => "Erroring".to_string(),
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
            user_count,
            total_members,
        })
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct StateResp {
    ok: bool,
    data: Option<Value>,
    error: Option<String>,
}

impl StateResp {
    fn into_value(self) -> Result<Option<Value>, Error> {
        if let Some(err) = self.error {
            return Err(format!("Sandwich proxy error: {}", err).into());
        }

        if self.ok {
            Ok(self.data)
        } else {
            Err(self.error.unwrap_or_else(|| "Unknown error".to_string()).into())
        }
    }
}

#[derive(Debug, Clone)]

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
    pub user_count: i64,
    pub total_members: i64,
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
    pub user_count: i64,
    pub member_count: i64
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
