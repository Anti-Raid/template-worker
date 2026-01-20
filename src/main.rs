mod api;
mod config;
mod data;
mod event_handler;
mod mesophyll;
mod fauxpas;
mod objectstore;
mod register;
mod sandwich;
mod worker;
mod migrations;

use crate::config::CONFIG;
use crate::data::Data;
use crate::event_handler::EventFramework;
use crate::mesophyll::client::{MesophyllClient, MesophyllDbClient};
use crate::mesophyll::server::DbState;
use crate::migrations::apply_migrations;
use crate::sandwich::Sandwich;
use crate::worker::workerdb::WorkerDB;
use crate::worker::workerlike::WorkerLike;
use crate::worker::workerpool::WorkerPool;
use crate::worker::workerprocesshandle::{WorkerProcessHandle, WorkerProcessHandleCreateOpts};
use crate::worker::workerstate::CreateWorkerState;
use crate::worker::workerthread::WorkerThread;
use clap::{Parser, ValueEnum};
use log::{error, info};
use serenity::all::{ApplicationId, HttpBuilder, UserId};
use sqlx::postgres::PgPoolOptions;
use std::io::Write;
use std::{sync::Arc, time::Duration};

pub type Error = Box<dyn std::error::Error + Send + Sync>; // This is constant and should be copy pasted

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum WorkerType {
    /// Dummy worker for registering commands only
    #[clap(name = "register", alias = "register-commands")]
    RegisterCommands,
    /// Dummy worker that applies a migration and exits
    #[clap(name = "migrate", alias = "apply-migration")]
    Migrate,
    /// Worker that uses a process pool for executing tasks
    #[clap(name = "processpool", alias = "process-pool")]
    ProcessPool,
    /// Worker that uses a thread pool for executing tasks
    #[clap(name = "threadpool", alias = "thread-pool")]
    ThreadPool,
    /// Single worker within a process pool system
    #[clap(name = "processpoolworker", alias = "process-pool-worker")]
    ProcessPoolWorker,
}

/// Command line arguments
#[derive(Parser, Debug, Clone)]
struct CmdArgs {
    /// Max connections that should be made to the database
    #[clap(long, default_value = "7")]
    pub max_db_connections: u32,

    #[clap(long, default_value_t = false)]
    pub use_tokio_console: bool,

    /// Number of threads to use for the worker thread pool
    #[clap(long, default_value = "30")]
    pub thread_workers: usize,

    #[clap(long, default_value = None)]
    pub process_workers: Option<usize>,

    /// Type of worker to use
    #[clap(long, default_value = "processpool", value_enum)]
    pub worker_type: WorkerType,

    /// The worker ID to use when running as a process pool worker
    ///
    /// Ignored unless `worker-type` is `processpoolworker`
    #[clap(long)]
    pub worker_id: Option<usize>,

    /// How many tokio threads to use for the master
    #[clap(long, default_value = "10")]
    pub tokio_threads_master: usize,

    /// How many tokio threads to use for the workers main loop (note that each worker still uses a single WorkerThread for the actual luau vm's)
    #[clap(long, default_value = "3")]
    pub tokio_threads_worker: usize,
}

/// Simple main function that initializes the tokio runtime and then calls the main (async) implementation
fn main() {
    let args = CmdArgs::parse();

    let num_tokio_threads = match args.worker_type {
        WorkerType::RegisterCommands => 1,
        WorkerType::Migrate => 1,
        WorkerType::ThreadPool => args.tokio_threads_master,
        WorkerType::ProcessPool => args.tokio_threads_master,
        WorkerType::ProcessPoolWorker => args.tokio_threads_worker,
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(num_tokio_threads)
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime");

    rt.block_on(async move {
        // Initialize the main implementation
        main_impl(args).await;
    });
}

async fn main_impl(args: CmdArgs) {
    let mut env_builder = env_logger::builder();

    if let Some(worker_id) = args.worker_id {
        // Make sure worker type is process pool worker
        if args.worker_type != WorkerType::ProcessPoolWorker {
            panic!("Worker ID can only be set when worker type is processpoolworker");
        }

        env_builder
            .format(move |buf, record| {
                writeln!(
                    buf,
                    "[Worker {}] ({}) {} - {}",
                    worker_id,
                    record.target(),
                    record.level(),
                    record.args()
                )
            })
            .filter(None, log::LevelFilter::Info);
    } else {
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
    }

    env_builder.init();

    if args.use_tokio_console {
        console_subscriber::init();
    }

    let proxy_url = CONFIG.meta.proxy.clone();

    info!("Proxy URL: {}", proxy_url);

    let token = serenity::all::SecretString::new(CONFIG.discord_auth.token.clone().into());
    let http = Arc::new(HttpBuilder::new(token.clone()).proxy(proxy_url).build());

    info!("HttpBuilder done");

    let client_builder = serenity::all::ClientBuilder::new_with_http(token, http.clone());

    info!("Connecting to database");

    let reqwest = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .expect("Could not initialize reqwest client");

    let sandwich = Sandwich::new(reqwest.clone(), http.clone());

    let current_user = loop {
        let current_user = match sandwich.current_user()
        .await {
            Ok(user) => user,
            Err(e) => {
                error!("Failed to get current user from Sandwich: {:?}, retrying in 5 seconds...", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        if current_user.id == UserId::new(0) {
            // TODO: Figure out why this happens sometimes
            log::error!("current_user.id == 0, this is a known bug with Sandwich, retrying in 5 seconds...");
            tokio::time::sleep(Duration::from_secs(5)).await;
            continue;
        }

        info!("Current user: {} ({})", current_user.name, current_user.id);

        http.set_application_id(ApplicationId::new(current_user.id.get()));
        break current_user;
    };

    let object_storage = Arc::new(
        CONFIG
            .object_storage
            .build()
            .expect("Could not initialize object store"),
    );

    match args.worker_type {
        WorkerType::RegisterCommands => {
            info!("Getting registration data from builtins");

            let data = &*register::REGISTER;

            println!("Register data: {:?}", data);

            http.create_global_commands(&data.commands)
                .await
                .expect("Failed to register commands");
        }
        WorkerType::Migrate => {
            let pg_pool = PgPoolOptions::new()
                .max_connections(args.max_db_connections)
                .connect(&CONFIG.meta.postgres_url)
                .await
                .expect("Could not initialize connection");

            apply_migrations(pg_pool).await.expect("Failed to apply migrations");
        }
        WorkerType::ThreadPool => {
            let pg_pool = PgPoolOptions::new()
                .max_connections(args.max_db_connections)
                .connect(&CONFIG.meta.postgres_url)
                .await
                .expect("Could not initialize connection");

            let db_state = DbState::new(pg_pool.clone())
            .await
            .expect("Failed to create DbState");

            let worker_state = CreateWorkerState::new(
                http.clone(),
                reqwest.clone(),
                object_storage.clone(),
                Arc::new(current_user.clone()),
                Arc::new(
                    WorkerDB::new_direct(
                        db_state.clone()
                    )
                ),
                sandwich.clone()
            );

            let worker_pool = Arc::new(
                WorkerPool::<WorkerThread>::new(args.thread_workers, &worker_state)
                    .expect("Failed to create worker thread pool"),
            );

            let data = Arc::new(Data {
                object_store: object_storage,
                reqwest,
                current_user,
                worker: worker_pool,
                sandwich: sandwich.clone()
            });

            let data1 = data.clone();
            let http1 = http.clone();
            let db_state1 = db_state.clone();
            tokio::task::spawn(async move {
                log::info!("Starting RPC server");

                let rpc_server = crate::api::server::create(data1, db_state1, pg_pool.clone(), http1);

                let listener = tokio::net::TcpListener::bind(&CONFIG.addrs.template_worker).await.unwrap();

                axum::serve(listener, rpc_server).await.unwrap();
            });

            let mut client = client_builder
                .data(data)
                .event_handler(EventFramework {})
                .wait_time_between_shard_start(Duration::from_secs(0)) // Disable wait time between shard start due to Sandwich
                .await
                .expect("Error creating client");

            info!("Starting using autosharding");

            if let Err(why) = client.start_autosharded().await {
                error!("Client error: {:?}", why);
                std::process::exit(1); // Clean exit with status code of 1
            }
        }
        WorkerType::ProcessPool => {
            // Ask sandwich how many shards we should use
            let shards = loop {
                let shard_count = match sandwich.get_shard_count().await {
                    Ok(count) => count,
                    Err(e) => {
                        error!("Failed to get shard count from Sandwich: {:?}, retrying in 5 seconds...", e);
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    }
                };

                break shard_count;
            };

            let pg_pool = PgPoolOptions::new()
                .max_connections(args.max_db_connections)
                .connect(&CONFIG.meta.postgres_url)
                .await
                .expect("Could not initialize connection");

            let mesophyll_server = mesophyll::server::MesophyllServer::new(
                CONFIG.addrs.mesophyll_server.clone(),
                shards,
                pg_pool.clone()
            )
            .await
            .expect("Failed to create Mesophyll server");
            let db_state = mesophyll_server.db_state().clone();

            let worker_pool = Arc::new(
                WorkerPool::<WorkerProcessHandle>::new(
                    shards,
                    &WorkerProcessHandleCreateOpts::new(mesophyll_server),
                )
                .expect("Failed to create worker thread pool"),
            );

            let data = Arc::new(Data {
                object_store: object_storage,
                reqwest,
                current_user,
                worker: worker_pool.clone(),
                sandwich: sandwich.clone()
            });

            let data1 = data.clone();
            let http1 = http.clone();
            let db_state1 = db_state.clone();
            tokio::task::spawn(async move {
                log::info!("Starting RPC server");

                let rpc_server = crate::api::server::create(data1, db_state1, pg_pool.clone(), http1);

                let listener = tokio::net::TcpListener::bind(&CONFIG.addrs.template_worker).await.unwrap();

                axum::serve(listener, rpc_server).await.unwrap();
            });

            // Loop indefinitely until Ctrl+C is pressed
            #[allow(clippy::never_loop)] // loop here is for documenting semantics
            loop {
                // On Unix, listen for *both* SIGINT and SIGTERM
                #[cfg(unix)]
                {
                    use tokio::signal::unix::{signal, SignalKind};

                    let mut sigint =
                        signal(SignalKind::interrupt()).expect("Failed to set up SIGINT handler");
                    let mut sigterm =
                        signal(SignalKind::terminate()).expect("Failed to set up SIGTERM handler");

                    tokio::select! {
                        _ = sigint.recv() => {
                            // Kill the worker pool
                            info!("Received SIGINT, shutting down worker pool");
                            worker_pool.kill().await.expect("Failed to kill worker pool");
                            break; // Exit the loop
                        }
                        _ = sigterm.recv() => {
                            // Kill the worker pool
                            info!("Received SIGTERM, shutting down worker pool");
                            worker_pool.kill().await.expect("Failed to kill worker pool");
                            break; // Exit the loop
                        }
                    }
                }

                // Fallback for non-unix systems
                #[cfg(not(unix))]
                {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {
                            // Kill the worker pool
                            info!("Received Ctrl+C, shutting down worker pool");
                            worker_pool.kill().await.expect("Failed to kill worker pool");
                            break; // Exit the loop
                        }
                    }
                }
            }
        }
        WorkerType::ProcessPoolWorker => {
            let Some(worker_id) = args.worker_id else {
                panic!("Worker ID must be set when worker type is processpoolworker");
            };
            let Some(process_workers) = args.process_workers else {
                panic!("Process workers must be set when worker type is processpoolworker");
            };

            let ident_token = std::env::var("MESOPHYLL_CLIENT_TOKEN").expect("Failed to find ident token for mesophyll");

            let worker_state = CreateWorkerState::new(
                http.clone(),
                reqwest.clone(),
                object_storage.clone(),
                Arc::new(current_user.clone()),
                Arc::new(
                    WorkerDB::new_mesophyll(
                        MesophyllDbClient::new(CONFIG.addrs.mesophyll_server.clone(), worker_id, ident_token.clone())
                    )
                ),
                sandwich.clone()
            );

            let worker_thread = Arc::new(
                WorkerThread::new(
                    worker_state,
                    WorkerPool::<WorkerProcessHandle>::filter_for(worker_id, process_workers),
                    worker_id,
                )
                .expect("Failed to create worker thread"),
            );

            let _meso_client = MesophyllClient::new(CONFIG.addrs.mesophyll_server.clone(), ident_token, worker_thread.clone());

            let data = Arc::new(Data {
                object_store: object_storage,
                reqwest,
                current_user,
                worker: worker_thread,
                sandwich: sandwich.clone()
            });

            let mut client = client_builder
                .data(data)
                .event_handler(EventFramework {})
                .wait_time_between_shard_start(Duration::from_secs(0)) // Disable wait time between shard start due to Sandwich
                .await
                .expect("Error creating client");

            info!("Starting worker...");

            // Start the worker shard
            if let Err(why) = client
                .start_shard(
                    worker_id.try_into().unwrap(),
                    process_workers.try_into().unwrap(),
                )
                .await
            {
                error!("Client error: {:?}", why);
                std::process::exit(1); // Clean exit with status code of 1
            }
        }
    }
}
