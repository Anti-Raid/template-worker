use crate::event_handler::EventFramework;
use crate::props::Props;
use arc_swap::ArcSwap;
use clap::Parser;
use log::{error, info};
use serenity::all::HttpBuilder;
use silverpelt::data::Data;
use sqlx::postgres::PgPoolOptions;
use std::io::Write;
use std::{sync::Arc, time::Duration};

#[derive(Parser, Debug, Clone)]
pub struct CmdArgs {
    #[clap(long)]
    pub shards: Option<Vec<u16>>,
    #[clap(long)]
    pub shard_count: Option<u16>,
}

pub async fn start() {
    const POSTGRES_MAX_CONNECTIONS: u32 = 70; // max connections to the database, we don't need too many here

    let mut env_args = std::env::args().collect::<Vec<String>>();
    env_args.remove(1);

    let cmd_args = Arc::new(CmdArgs::parse_from(env_args));

    let mut env_builder = env_logger::builder();

    env_builder.format(move |buf, record| {
        writeln!(
            buf,
            "({}) {} - {}",
            record.target(),
            record.level(),
            record.args()
        )
    });

    env_builder.init();

    let proxy_url = config::CONFIG.meta.proxy.clone();

    info!("Proxy URL: {}", proxy_url);

    let http = Arc::new(
        HttpBuilder::new(&config::CONFIG.discord_auth.token)
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
        .max_connections(POSTGRES_MAX_CONNECTIONS)
        .connect(&config::CONFIG.meta.postgres_url)
        .await
        .expect("Could not initialize connection");

    let reqwest = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .expect("Could not initialize reqwest client");

    let props = Arc::new(Props {
        cache: ArcSwap::new(None.into()),
        shard_manager: ArcSwap::new(None.into()),
    });

    let data = Data {
        object_store: Arc::new(
            config::CONFIG
                .object_storage
                .build()
                .expect("Could not initialize object store"),
        ),
        pool: pg_pool.clone(),
        reqwest,
        extra_data: dashmap::DashMap::new(),
        props: props.clone(),
    };

    let mut client = client_builder
        .data(Arc::new(data))
        .framework(EventFramework {})
        .wait_time_between_shard_start(Duration::from_secs(0)) // Disable wait time between shard start due to Sandwich
        .await
        .expect("Error creating client");

    props.cache.store(Arc::new(Some(client.cache.clone())));
    props
        .shard_manager
        .store(Arc::new(Some(client.shard_manager.clone())));

    client.cache.set_max_messages(10000);

    if let Some(shard_count) = cmd_args.shard_count {
        if let Some(ref shards) = cmd_args.shards {
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

    info!("Starting using autosharding");

    if let Err(why) = client.start_autosharded().await {
        error!("Client error: {:?}", why);
        std::process::exit(1); // Clean exit with status code of 1
    }
}
