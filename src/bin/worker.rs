use log::debug;
use tokio::sync::watch;
use tw::config::CONFIG;
use tw::mesophyll::client::MesophyllClient;
use tw::setup_discord;
use tw::worker::workerstate::WorkerState;
use tw::worker::workerthread::WorkerThread;
use clap::Parser;
use std::io::Write;
use std::sync::Arc;

/// Command line arguments
#[derive(Parser, Debug, Clone)]
struct CmdArgs {
    /// Enables debug logging for luau in workers
    #[clap(long, default_value_t = false)]
    pub worker_debug: bool,

    /// The worker ID to use
    #[clap(long)]
    pub worker_id: usize,

    /// How many tokio threads to use for the workers main loop (note that each worker still uses a single WorkerThread for the actual luau vm's)
    #[clap(long, default_value = "3")]
    pub tokio_threads_worker: usize,
}

/// Simple main function that initializes the tokio runtime and then calls the main (async) implementation
fn main() {
    let args = CmdArgs::parse();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(args.tokio_threads_worker)
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

    let worker_id = args.worker_id;
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

    let (http, stratum, current_user) = setup_discord().await;

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
    unreachable!("stratum unexpectedly closed");
}
