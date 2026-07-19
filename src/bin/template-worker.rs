use tw::config::CONFIG;
use tw::master::syscall::MSyscallHandler;
use tw::master::workerpool::WorkerPool;
use tw::setup_discord;
use log::{debug, info};
use sqlx::postgres::PgPoolOptions;
use std::io::Write;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};

/// Command line arguments
#[derive(Debug, Clone)]
struct CmdArgs {
    /// Max connections that should be made to the database
    pub max_db_connections: u32,

    /// Enables debug logging for luau in workers
    pub worker_debug: bool,

    /// How many tokio threads to use for the master
    pub tokio_threads: usize,
}

impl CmdArgs {
    const MAX_DB_CONNECTIONS: u32 = 7;
    const TOKIO_THREADS: usize = 10;
    const WORKER_DEBUG: bool = false;
    pub fn parse() -> Self {
        let max_db_connections = std::env::var("MAX_DB_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::MAX_DB_CONNECTIONS);
        let tokio_threads = std::env::var("TOKIO_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::TOKIO_THREADS);
        let worker_debug = std::env::var("WORKER_DEBUG")
            .ok()
            .and_then(|s| Some(s.to_lowercase() == "true" || s == "1"))
            .unwrap_or(Self::WORKER_DEBUG);
        Self { max_db_connections, tokio_threads, worker_debug }
    }
}

/// Simple main function that initializes the tokio runtime and then calls the main (async) implementation
fn main() {
    let args = CmdArgs::parse();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(args.tokio_threads)
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

    debug!("Connecting to database");

    let reqwest = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(30))
        .timeout(std::time::Duration::from_secs(90))
        .build()
        .expect("Could not initialize reqwest client");

    let stratum = setup_discord().await;

    // Ask stratum for its worker count
    let worker_count: usize = stratum.get_config()
    .await
    .expect("Failed to get worker count")
    .num_workers
    .try_into()
    .expect("worker_count exceeds usize limits");

    let pg_pool = PgPoolOptions::new()
        .max_connections(args.max_db_connections)
        .connect(&CONFIG.postgres_url)
        .await
        .expect("Could not initialize connection");

    let mesophyll_server = tw::mesophyll::server::MesophyllServer::new(
        worker_count,
        pg_pool.clone()
    )
    .await
    .expect("Failed to create Mesophyll server");

    let worker_pool = Arc::new(
        WorkerPool::new(worker_count, args.worker_debug, mesophyll_server.clone())
    );
    
    // Start msyscall server
    let msyscall_handler = MSyscallHandler::new(
        worker_pool.clone(),
        stratum,
        reqwest,
        pg_pool,
    );

    mesophyll_server.set_msyscall_handler(msyscall_handler.clone()).unwrap();

    tokio::task::spawn(async move {
        log::info!("Starting RPC server");

        let rpc_server = tw::master::syscall::webapi::create(msyscall_handler);
        let listener = tokio::net::TcpListener::bind(&CONFIG.template_worker_bind_addr).await.unwrap();
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
            worker_pool.shutdown_all().await.expect("Failed to kill worker pool");
            worker_pool.mesophyll().sock_file().drop_full();
        }
        _ = sigterm.recv() => {
            // Kill the worker pool
            info!("Received SIGTERM, shutting down worker pool");
            worker_pool.shutdown_all().await.expect("Failed to kill worker pool");
        }
    }
}
