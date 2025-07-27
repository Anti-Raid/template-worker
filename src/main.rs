mod config;
mod data;
mod dispatch;
mod event_handler;
mod expiry_tasks;
mod internalapi;
mod jobserver;
mod objectstore;
mod register;
mod sandwich;
mod templatingrt;

use crate::config::{CMD_ARGS, CONFIG};
use crate::data::Data;
use crate::event_handler::EventFramework;
use crate::templatingrt::cache::setup;
use log::{error, info};
use serenity::all::{ApplicationId, HttpBuilder};
use sqlx::postgres::PgPoolOptions;
use std::io::Write;
use std::str::FromStr;
use std::{sync::Arc, time::Duration};

pub type Error = Box<dyn std::error::Error + Send + Sync>; // This is constant and should be copy pasted

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

    if CMD_ARGS.use_tokio_console {
        console_subscriber::init();
    }

    let proxy_url = CONFIG.meta.proxy.clone();

    info!("Proxy URL: {}", proxy_url);

    let token = serenity::all::Token::from_str(&CONFIG.discord_auth.token).expect("Failed to validate token");
    let http = Arc::new(
        HttpBuilder::new(token.clone())
            .proxy(proxy_url)
            .build(),
    );

    info!("HttpBuilder done");

    let client_builder = serenity::all::ClientBuilder::new_with_http(token, http);

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

    let current_user = sandwich::current_user(&reqwest)
        .await
        .expect("Failed to get current user");

    let current_user_id = current_user.id;

    info!("Current user: {} ({})", current_user.name, current_user_id);

    let data = Data {
        object_store: Arc::new(
            CONFIG
                .object_storage
                .build()
                .expect("Could not initialize object store"),
        ),
        pool: pg_pool.clone(),
        reqwest,
        current_user
    };

    let mut client = client_builder
        .data(Arc::new(data))
        .event_handler(EventFramework {})
        .wait_time_between_shard_start(Duration::from_secs(0)) // Disable wait time between shard start due to Sandwich
        .await
        .expect("Error creating client");

    info!("Getting registration data from builtins");

    let data = &*register::REGISTER;

    println!("Register data: {:?}", data);

    client.http.set_application_id(ApplicationId::new(current_user_id.get()));

    if CMD_ARGS.register_commands_only {
        client
            .http
            .create_global_commands(&data.commands)
            .await
            .expect("Failed to register commands");

        return;
    }

    info!("Setting up template cache");

    setup(&pg_pool)
        .await
        .expect("Failed to setup template cache");

    if let Some(shard_count) = CMD_ARGS.shard_count {
        if let Some(ref shards) = CMD_ARGS.shards {
            let shard_range = std::ops::Range {
                start: shards[0],
                end: *shards.last().expect("Shards should not be empty"),
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
    } else {
        info!("Starting using autosharding");

        if let Err(why) = client.start_autosharded().await {
            error!("Client error: {:?}", why);
            std::process::exit(1); // Clean exit with status code of 1
        }
    }
}
