use std::sync::Arc;

use serde_json::Value;
use serenity::all::{GenericChannelId, GuildId, ResultJson, UserId};
use stratum_client::{BulkIsResourceInCacheRequest, GetResourceRequest, StratumClient};
use stratum_common::{GuildFetchOpts, pb};
use tokio::sync::watch;

use crate::{Error, config::CONFIG, worker::{workerdispatch::SimpleEvent, workerthread::WorkerThread, workervmmanager::Id}};

#[derive(Clone)]
pub struct Stratum {
    client: Arc<StratumClient>,
    http: Arc<serenity::http::Http>,
}

impl std::ops::Deref for Stratum {
    type Target = StratumClient;

    fn deref(&self) -> &Self::Target {
        &self.client
    }
}

impl Stratum {
    pub async fn new(http: Arc<serenity::http::Http>) -> Result<Self, Error> {
        let client = StratumClient::new(&CONFIG.meta.stratum_server, CONFIG.meta.stratum_grpc_access_key.clone()).await?;
        Ok(Self { client: Arc::new(client), http })
    }

    /// Starts listening for discord events and pushing them to worker thread
    pub async fn listen_discord_events(&self, wt: Arc<WorkerThread>, shutdown: watch::Receiver<bool>) {
        loop {
            if *shutdown.borrow() {
                return;
            }
            match self.listen_discord_events_impl(wt.clone(), shutdown.clone()).await {
                Ok(_) => break,
                Err(e) => {
                    log::error!("[Worker {wid}] Error in stream {e:?}, retrying in 5 seconds", wid=wt.id());
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    /// Helper method to start the event stream and listen in calling `discord_event_dispatch` for every message
    async fn listen_discord_events_impl(&self, wt: Arc<WorkerThread>, shutdown: watch::Receiver<bool>) -> Result<(), crate::Error> {
        let Some(current_user) = self.current_user().await? else {
            return Err("Current user not ready yet!".into());
        };
        let bot_id = current_user.id;

        let stream = self.event_stream(wt.id().try_into()?).await?;
        log::info!("[Worker {wid}] Started event stream", wid=wt.id());
        self.listen_to_stream(stream, Some(shutdown), move |evt| {
            //log::info!("[Worker {wid}] Got event: {} json_ok({})", evt.event_name, value.is_ok());
            if let Err(e) = Self::discord_event_dispatch(&wt, bot_id, evt) {
                log::error!("Error dispatching event: {:?}", e);
            }
            false
        }).await?;
        Ok(())
    }

    fn discord_event_dispatch(
        wt: &WorkerThread,
        bot_id: UserId,
        evt: pb::DiscordEvent,
    ) -> Result<(), crate::Error> {
        log::info!("Event: {}, gid: {}, target_user: {}, payload: {}", evt.event_name, evt.guild_id, evt.target_user, evt.payload);
        
        let id = if evt.guild_id != 0 && evt.guild_id != u64::MAX {
            Id::Guild(GuildId::new(evt.guild_id))
        } else if evt.target_user != 0 {
            Id::User(UserId::new(evt.target_user)) // User installed app
        } else {
            return Ok(()); // No routing info
        };

        if evt.msg_author != 0 && evt.msg_author == bot_id.get() {
            return Ok(()); // avoid self-bot related footguns
        }  

        wt.dispatch_event_nowait(
            id,
            SimpleEvent::new_json_string(
                evt.event_name,
                None,
                evt.payload,
            ),
        )?;

        Ok(())
    }

    /// Helper method to extract value or return None from a serenity http response
    fn extract_from_discord(val: ResultJson) -> Result<Option<Value>, Error> {
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

    /// Returns the current user
    pub async fn current_user(&self) -> Result<Option<serenity::all::CurrentUser>, Error> {
        self.client.get_parsed_resource_from_cache::<_>(GetResourceRequest::CurrentUser {}).await
    }

    /// Given a list of guild ids, returns which ones the bot is in
    pub async fn has_guilds(
        &self,
        guilds: &[GuildId],
    ) -> Result<Vec<bool>, Error> {
        let guilds = guilds.iter().map(|x| x.get()).collect::<Vec<_>>();
        let resp = self.client.bulk_is_resource_in_cache(BulkIsResourceInCacheRequest::Guild {
            guild_id: guilds
        }).await?;
        Ok(resp.cached)
    }

    /// Fetches a guild, trying first Stratum and then the discord api
    pub async fn guild(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<Value>, Error> {
        // First try normal fetch
        if let Some(guild) = self.get_resource_from_cache(GetResourceRequest::Guild { guild_id: guild_id.get(), flags: GuildFetchOpts::empty() }).await? {
            return Ok(Some(guild));
        }

        // Last resort: make the http call
        let res = self.http.get_guild_with_counts(guild_id).await;
        Self::extract_from_discord(res)
    }

    /// Fetches a member in a guild, trying first Stratum and then the discord api
    pub async fn guild_member(
        &self,
        guild_id: GuildId,
        user_id: UserId,
    ) -> Result<Option<Value>, Error> {
        // First try normal fetch
        if let Some(member) = self.get_resource_from_cache(GetResourceRequest::GuildMember { guild_id: guild_id.get(), user_id: user_id.get() }).await? {
            return Ok(Some(member));
        }

        // Last resort: make the http call
        let res = self.http.get_member(guild_id, user_id).await;
        Self::extract_from_discord(res)
    }

    /// Fetches all guild roles, trying first Stratum and then the discord api
    pub async fn guild_roles(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<Value>, Error> {
        // First try normal fetch
        if let Some(roles) = self.get_resource_from_cache(GetResourceRequest::GuildRoles { guild_id: guild_id.get() }).await? {
            return Ok(Some(roles));
        }

        // Last resort: make the http call
        let res = self.http.get_guild_roles(guild_id).await;
        Self::extract_from_discord(res)
    }

    /// Fetches all guild channels, trying first Stratum and then the discord api
    pub async fn guild_channels(
        &self,
        guild_id: GuildId,
    ) -> Result<Option<Value>, Error> {
        // First try normal fetch
        if let Some(channels) = self.get_resource_from_cache(GetResourceRequest::GuildChannels { guild_id: guild_id.get() }).await? {
            return Ok(Some(channels));
        }

        // Last resort: make the http call
        let res = self.http.get_channels(guild_id).await;
        Self::extract_from_discord(res)
    }

    /// Fetches a channel, trying first Stratum and then the discord api
    pub async fn channel(
        &self,
        channel_id: GenericChannelId,
    ) -> Result<Option<Value>, Error> {
        // First try normal fetch
        if let Some(channel) = self.get_resource_from_cache(GetResourceRequest::Channel { channel_id: channel_id.get() }).await? {
            return Ok(Some(channel));
        }

        // Last resort: make the http call
        let res = self.http.get_channel(channel_id).await;
        Self::extract_from_discord(res)
    }
}