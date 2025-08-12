mod config;
mod data;
mod dispatch;
mod event_handler;
mod api;
mod objectstore;
mod register;
mod sandwich;
mod events;
mod worker;

use crate::config::CONFIG;
use crate::data::Data;
use crate::event_handler::EventFramework;
use crate::worker::workerlike::WorkerLike;
use crate::worker::workerprocesscomm::WorkerProcessCommClient;
use crate::worker::workerprocesscommhttp2::{WorkerProcessCommHttp2ServerCreator, WorkerProcessCommHttp2Worker};
use crate::worker::workerprocesshandle::{WorkerProcessHandle, WorkerProcessHandleCreateOpts};
use crate::worker::workerstate::WorkerState;
use crate::worker::workerpool::WorkerPool;
use crate::worker::workerthread::WorkerThread;
use log::{error, info};
use serenity::all::{ApplicationId, HttpBuilder};
use sqlx::postgres::PgPoolOptions;
use std::io::Write;
use std::{sync::Arc, time::Duration};
use clap::{Parser, ValueEnum};

pub type Error = Box<dyn std::error::Error + Send + Sync>; // This is constant and should be copy pasted

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
pub enum WorkerType {
    /// Dummy worker for registering commands only
    #[clap(name = "register", alias = "register-commands")]
    RegisterCommands,
    /// Worker that uses a thread pool for executing tasks
    #[clap(name = "threadpool", alias = "thread-pool")]
    ThreadPool,
    /// Worker that uses a process pool for executing tasks
    #[clap(name = "processpool", alias = "process-pool")]
    ProcessPool,
    /// Single worker within a process pool system
    #[clap(name = "processpoolworker", alias = "process-pool-worker")]
    ProcessPoolWorker
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
    pub worker_threads: usize,

    /// Type of worker to use
    #[clap(long, default_value = "processpool", value_enum)]
    pub worker_type: WorkerType,

    /// The worker ID to use when running as a process pool worker
    /// 
    /// Ignored unless `worker-type` is `processpoolworker`
    #[clap(long)]
    pub worker_id: Option<usize>,

    /// The worker process communication type to use when running as a process pool worker
    /// 
    /// Ignored unless `worker-type` is `processpoolworker`
    #[clap(long)]
    pub worker_comm_type: Option<String>,

    /// How many db connections should each worker within the process pool have
    #[clap(long, default_value = "3")]
    pub max_worker_db_connections: usize,

    /// How many tokio threads to use for the master
    #[clap(long, default_value = "10")]
    pub tokio_threads_master: usize,

    /// How many tokio threads to use for the workers
    #[clap(long, default_value = "3")]
    pub tokio_threads_worker: usize,
}

/// Simple main function that initializes the tokio runtime and then calls the main (async) implementation
fn main() {
    let args = CmdArgs::parse();

    let num_tokio_threads = match args.worker_type {
        WorkerType::RegisterCommands => 1,
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
    let http = Arc::new(
        HttpBuilder::new(token.clone())
            .proxy(proxy_url)
            .build(),
    );

    info!("HttpBuilder done");

    let client_builder = serenity::all::ClientBuilder::new_with_http(token, http.clone());

    info!("Connecting to database");

    let pg_pool = PgPoolOptions::new()
        .max_connections(args.max_db_connections)
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

    http.set_application_id(ApplicationId::new(current_user_id.get()));

    let object_storage = Arc::new(
        CONFIG
            .object_storage
            .build()
            .expect("Could not initialize object store"),
    );

    let worker_state = WorkerState::new(
        http.clone(),
        reqwest.clone(),
        object_storage.clone(),
        pg_pool.clone(),
        Arc::new(current_user.clone()),
    ).expect("Failed to create worker state");

    match args.worker_type {
        WorkerType::RegisterCommands => {
            info!("Getting registration data from builtins");

            let data = &*register::REGISTER;

            println!("Register data: {:?}", data);

            http
                .create_global_commands(&data.commands)
                .await
                .expect("Failed to register commands");

        },
        WorkerType::ThreadPool => {
            let worker_pool = Arc::new(WorkerPool::<WorkerThread>::new(
                worker_state.clone(),
                args.worker_threads,
                &()
            )
            .expect("Failed to create worker thread pool"));

            let data = Arc::new(Data {
                object_store: object_storage,
                pool: pg_pool.clone(),
                reqwest,
                current_user,
                worker: worker_pool,
            });

            let data1 = data.clone();
            let http1 = http.clone();
            tokio::task::spawn(async move {
                log::info!("Starting RPC server");

                let rpc_server = crate::api::server::create(data1, http1);

                let addr = format!(
                    "{}:{}",
                    CONFIG.base_ports.template_worker_bind_addr,
                    CONFIG.base_ports.template_worker_port
                );

                let listener = tokio::net::TcpListener::bind(addr)
                    .await
                    .unwrap();

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
        },
        WorkerType::ProcessPool => {
            let worker_pool = Arc::new(WorkerPool::<WorkerProcessHandle>::new(
                worker_state.clone(),
                args.worker_threads,
                &WorkerProcessHandleCreateOpts::new(
                    Arc::new(WorkerProcessCommHttp2ServerCreator::new(
                        reqwest::Client::builder()
                        .http2_prior_knowledge()
                        .build()
                        .expect("Failed to create reqwest client for worker process communication"),
                    )),
                    args.max_worker_db_connections,
                )
            )
            .expect("Failed to create worker thread pool"));

            let data = Arc::new(Data {
                object_store: object_storage,
                pool: pg_pool.clone(),
                reqwest,
                current_user,
                worker: worker_pool.clone(),
            });

            let data1 = data.clone();
            let http1 = http.clone();
            tokio::task::spawn(async move {
                log::info!("Starting RPC server");

                let rpc_server = crate::api::server::create(data1, http1);

                let addr = format!(
                    "{}:{}",
                    CONFIG.base_ports.template_worker_bind_addr,
                    CONFIG.base_ports.template_worker_port
                );

                let listener = tokio::net::TcpListener::bind(addr)
                    .await
                    .unwrap();

                axum::serve(listener, rpc_server).await.unwrap();
            });

            // Loop indefinitely until Ctrl+C is pressed
            loop {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        // Kill the worker pool
                        info!("Received Ctrl+C, shutting down worker pool");
                        worker_pool.kill().await.expect("Failed to kill worker pool");
                        break; // Exit the loop
                    }
                }
            }
        },
        WorkerType::ProcessPoolWorker => {
            let Some(worker_id) = args.worker_id else {
                panic!("Worker ID must be set when worker type is processpoolworker");
            };

            let worker_thread = Arc::new(WorkerThread::new(
                worker_state.clone(),
                WorkerPool::<WorkerProcessHandle>::filter_for(worker_id, args.worker_threads),
                worker_id
            ).expect("Failed to create worker thread"));

            let Some(worker_comm_type) = args.worker_comm_type else {
                panic!("Worker comm type must be set when worker type is processpoolworker");
            };

            let _comm_client: Arc<dyn WorkerProcessCommClient> = match worker_comm_type.as_str() {
                "http2" => Arc::new(WorkerProcessCommHttp2Worker::new(worker_thread.clone()).await.expect("Failed to create HTTP/2 worker process communication client")),
                _ => panic!("Unknown worker comm type: {}", worker_comm_type),
            };

            let data = Arc::new(Data {
                object_store: object_storage,
                pool: pg_pool.clone(),
                reqwest,
                current_user,
                worker: worker_thread,
            });

            let mut client = client_builder
                .data(data)
                .event_handler(EventFramework {})
                .wait_time_between_shard_start(Duration::from_secs(0)) // Disable wait time between shard start due to Sandwich
                .await
                .expect("Error creating client");

            info!("Starting worker...");

            // Start the worker shard
            if let Err(why) = client.start_shard(worker_id.try_into().unwrap(), args.worker_threads.try_into().unwrap()).await {
                error!("Client error: {:?}", why);
                std::process::exit(1); // Clean exit with status code of 1
            }   
        }
    }
}
