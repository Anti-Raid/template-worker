mod config;
mod mesophyll;
mod worker;
mod migrations;
mod geese;
mod master;

use crate::config::CONFIG;
use crate::master::{shell, register};
use crate::master::syscall::MSyscallHandler;
use crate::mesophyll::client::{MesophyllClient, MesophyllShellClient};
use crate::migrations::apply_migrations;
use crate::geese::stratum::Stratum;
use crate::master::workerpool::WorkerPool;
use crate::worker::workerstate::WorkerState;
use crate::worker::workerthread::WorkerThread;
use clap::{Parser, ValueEnum};
use log::{debug, error, info};
use serenity::all::{ApplicationId, CurrentUser, Http, HttpBuilder};
use sqlx::postgres::PgPoolOptions;
use tokio::sync::watch;
use std::io::Write;
use std::{sync::Arc, time::Duration};
use tokio::signal::unix::{signal, SignalKind};

pub type Error = Box<dyn std::error::Error + Send + Sync>; // This is constant and should be copy pasted

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum WorkerType {
    /// Dummy worker for registering commands only
    #[clap(name = "register", alias = "register-commands")]
    RegisterCommands,
    /// Dummy worker that applies a migration and exits
    #[clap(name = "migrate", alias = "apply-migration")]
    Migrate,
    /// Dummy worker that spawns a fauxpas shell instead of running the worker
    #[clap(name = "shell", alias = "fauxpas")]
    Shell,
    /// Worker that uses a process pool for executing tasks
    #[clap(name = "processpool", alias = "process-pool")]
    ProcessPool,
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

    /// Enables debug logging for luau in workers
    #[clap(long, default_value_t = false)]
    pub worker_debug: bool,

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
        WorkerType::Shell => args.tokio_threads_master,
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

    debug!("Connecting to database");

    let reqwest = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .expect("Could not initialize reqwest client");

    let object_storage = Arc::new(
        CONFIG
            .object_storage
            .build()
            .expect("Could not initialize object store"),
    );

    match args.worker_type {
        WorkerType::RegisterCommands => {
            let (http, _, _) = setup_discord().await;

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
        WorkerType::Shell => {            
            shell::init_shell(MesophyllShellClient::new().await.expect("failed to create meso shell client"));
        }
        WorkerType::ProcessPool => {
            let (_, stratum, current_user) = setup_discord().await;

            // Ask stratum for its worker count
            let worker_count: usize = stratum.get_config()
            .await
            .expect("Failed to get worker count")
            .num_workers
            .try_into()
            .expect("worker_count exceeds usize limits");

            let pg_pool = PgPoolOptions::new()
                .max_connections(args.max_db_connections)
                .connect(&CONFIG.meta.postgres_url)
                .await
                .expect("Could not initialize connection");

            let mesophyll_server = mesophyll::server::MesophyllServer::new(
                worker_count,
                pg_pool.clone()
            )
            .await
            .expect("Failed to create Mesophyll server");

            let worker_pool = Arc::new(
                WorkerPool::new(worker_count, args.worker_debug, &mesophyll_server)
                .expect("Failed to create worker process pool"),
            );
            
            // Start msyscall server
            let msyscall_handler = MSyscallHandler::new(
                current_user.into(),
                worker_pool.clone(),
                stratum,
                reqwest,
                pg_pool
            );

            mesophyll_server.set_msyscall_handler(msyscall_handler.clone()).unwrap();

            tokio::task::spawn(async move {
                log::info!("Starting RPC server");

                let rpc_server = master::syscall::webapi::create(msyscall_handler);
                let listener = tokio::net::TcpListener::bind(&CONFIG.addrs.template_worker).await.unwrap();
                axum::serve(listener, rpc_server).await.unwrap();
            });

            // Wait indefinitely until Ctrl+C is pressed
            let mut sigint =
                signal(SignalKind::interrupt()).expect("Failed to set up SIGINT handler");
            let mut sigterm =
                signal(SignalKind::terminate()).expect("Failed to set up SIGTERM handler");

            tokio::select! {
                _ = sigint.recv() => {
                    // Kill the worker pool
                    info!("Received SIGINT, shutting down worker pool");
                    worker_pool.kill().await.expect("Failed to kill worker pool");
                }
                _ = sigterm.recv() => {
                    // Kill the worker pool
                    info!("Received SIGTERM, shutting down worker pool");
                    worker_pool.kill().await.expect("Failed to kill worker pool");
                }
            }
        }
        WorkerType::ProcessPoolWorker => {
            let (http, stratum, current_user) = setup_discord().await;

            let Some(worker_id) = args.worker_id else {
                panic!("Worker ID must be set when worker type is processpoolworker");
            };

            let (meso_client, meso_client_stream) = MesophyllClient::new(worker_id)
                .await
                .expect("Failed to create Mesophyll client");

            let worker_state = WorkerState::new(
                http.clone(),
                object_storage.clone(),
                Arc::new(current_user.clone()),
                Arc::new(meso_client.clone()),
                stratum.clone(),
                reqwest,
                args.worker_debug
            );

            let worker_thread = Arc::new(
                WorkerThread::new(
                    worker_state,
                    worker_id,
                )
                .expect("Failed to create worker thread"),
            );

            // Start listening to the mesophyll server stream for events to dispatch to this worker thread
            meso_client.listen(meso_client_stream, worker_thread.clone());

            // Start listening to stratum stream
            let (_shutdown_tx, shutdown_rx) = watch::channel(false);
            stratum.listen_discord_events(worker_thread, shutdown_rx).await;
        }
    }
}

async fn setup_discord() -> (Arc<Http>, Stratum, CurrentUser) {
    let proxy_url = CONFIG.meta.proxy.clone();

    debug!("Proxy URL: {}", proxy_url);

    let token = serenity::all::SecretString::new(CONFIG.discord_auth.token.clone().into());
    let http = Arc::new(HttpBuilder::new(token.clone()).proxy(proxy_url).build());

    let stratum = Stratum::new(http.clone()).await.expect("Failed to connect to stratum");

    let current_user = loop {
        match stratum.current_user().await {
            Ok(Some(user)) => break user,
            Ok(None) => {
                error!("Current user is not available yet, retrying in 5 seconds...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Err(e) => {
                error!("Failed to get current user from Sandwich: {:?}, retrying in 5 seconds...", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        }
    };

    debug!("Current user: {} ({})", current_user.name, current_user.id);
    http.set_application_id(ApplicationId::new(current_user.id.get()));

    (http, stratum, current_user)
}