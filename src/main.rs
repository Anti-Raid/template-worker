mod config;
mod dispatch;
mod event_handler;
mod expiry_tasks;
mod http;
mod pages;
mod serenitystore;
mod templatingrt;
mod temporary_punishments;

use crate::config::{CMD_ARGS, CONFIG};
use crate::event_handler::EventFramework;
use crate::templatingrt::cache::setup;
use log::{error, info};
use serenity::all::HttpBuilder;
use silverpelt::data::Data;
use sqlx::postgres::PgPoolOptions;
use std::io::Write;
use std::{sync::Arc, time::Duration};

/// The main function is just a command handling function
#[tokio::main]
async fn main() {
    let _ = &*CMD_ARGS;

    let mut env_builder = env_logger::builder();

    env_builder
        .format(move |buf, record| {
            writeln!(
                buf,
                "({}) {} - {}",
                record.target(),
                record.level(),
                record.args()
            )
        })
        .filter(None, log::LevelFilter::Info);

    env_builder.init();

    let proxy_url = CONFIG.meta.proxy.clone();

    info!("Proxy URL: {}", proxy_url);

    let http = Arc::new(
        HttpBuilder::new(&CONFIG.discord_auth.token)
            .proxy(proxy_url)
            .ratelimiter_disabled(true)
            .build(),
    );

    info!("HttpBuilder done");

    let mut intents = serenity::all::GatewayIntents::all();

    // Remove the really spammy intents
    intents.remove(serenity::all::GatewayIntents::GUILD_PRESENCES); // Don't even have the privileged gateway intent for this
    intents.remove(serenity::all::GatewayIntents::GUILD_MESSAGE_TYPING); // Don't care about typing
    intents.remove(serenity::all::GatewayIntents::DIRECT_MESSAGE_TYPING); // Don't care about typing
    intents.remove(serenity::all::GatewayIntents::DIRECT_MESSAGES); // Don't care about DMs

    let client_builder = serenity::all::ClientBuilder::new_with_http(http, intents);

    info!("Connecting to database");

    let pg_pool = PgPoolOptions::new()
        .max_connections(CMD_ARGS.max_db_connections)
        .connect(&CONFIG.meta.postgres_url)
        .await
        .expect("Could not initialize connection");

    let reqwest = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .expect("Could not initialize reqwest client");

    let data = Data {
        object_store: Arc::new(
            CONFIG
                .object_storage
                .build()
                .expect("Could not initialize object store"),
        ),
        pool: pg_pool.clone(),
        reqwest,
    };

    let mut client = client_builder
        .data(Arc::new(data))
        .framework(EventFramework {})
        .wait_time_between_shard_start(Duration::from_secs(0)) // Disable wait time between shard start due to Sandwich
        .await
        .expect("Error creating client");

    client.cache.set_max_messages(10000);

    if let Some(shard_count) = CMD_ARGS.shard_count {
        if let Some(ref shards) = CMD_ARGS.shards {
            let shard_range = std::ops::Range {
                start: shards[0],
                end: *shards.last().unwrap(),
            };

            info!("Starting shard range: {:?}", shard_range);

            if let Err(why) = client.start_shard_range(shard_range, shard_count).await {
                error!("Client error: {:?}", why);
                std::process::exit(1); // Clean exit with status code of 1
            }

            return;
        } else {
            info!("Starting shard count: {}", shard_count);

            if let Err(why) = client.start_shards(shard_count).await {
                error!("Client error: {:?}", why);
                std::process::exit(1); // Clean exit with status code of 1
            }

            return;
        }
    }

    info!("Setting up template cache");

    setup(&pg_pool)
        .await
        .expect("Failed to setup template cache");

    info!("Starting using autosharding");

    if let Err(why) = client.start_autosharded().await {
        error!("Client error: {:?}", why);
        std::process::exit(1); // Clean exit with status code of 1
    }
}
