use tw::config::CONFIG;
use tw::master::syscall::MSyscallHandler;
use tw::master::workerpool::WorkerPool;
use tw::setup_discord;
use clap::Parser;
use log::{debug, info};
use sqlx::postgres::PgPoolOptions;
use std::io::Write;
use std::sync::Arc;
use tokio::signal::unix::{signal, SignalKind};

/// Command line arguments
#[derive(Parser, Debug, Clone)]
struct CmdArgs {
    /// Max connections that should be made to the database
    #[clap(long, default_value = "7")]
    pub max_db_connections: u32,

    /// Enables debug logging for luau in workers
    #[clap(long, default_value_t = false)]
    pub worker_debug: bool,

    /// How many tokio threads to use for the master
    #[clap(long, default_value = "10")]
    pub tokio_threads_master: usize,
}

/// Simple main function that initializes the tokio runtime and then calls the main (async) implementation
fn main() {
    let args = CmdArgs::parse();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(args.tokio_threads_master)
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

    let object_storage = Arc::new(
        CONFIG
            .object_storage
            .build()
            .expect("Could not initialize object store"),
    );

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

    let mesophyll_server = tw::mesophyll::server::MesophyllServer::new(
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
        pg_pool,
        object_storage
    );

    mesophyll_server.set_msyscall_handler(msyscall_handler.clone()).unwrap();

    tokio::task::spawn(async move {
        log::info!("Starting RPC server");

        let rpc_server = tw::master::syscall::webapi::create(msyscall_handler);
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
