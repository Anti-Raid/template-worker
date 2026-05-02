use log::debug;
use tokio::sync::watch;
use tw::mesophyll::client::MesophyllClient;
use tw::setup_discord;
use tw::worker::workerstate::WorkerState;
use tw::worker::workerthread::WorkerThread;
use std::io::Write;
use std::process::exit;
use std::sync::Arc;

/// Command line arguments
#[derive(Debug, Clone)]
struct CmdArgs {
    /// Enables debug logging for luau in workers
    pub worker_debug: bool,

    /// The worker ID to use
    pub worker_id: usize,

    /// How many tokio threads to use for the workers main loop (note that each worker still uses a single WorkerThread for the actual luau vm's)
    pub tokio_threads: usize,
}

impl CmdArgs {
    const TOKIO_THREADS: usize = 3;
    const WORKER_DEBUG: bool = false;
    pub fn parse() -> Self {
        let args = std::env::args().collect::<Vec<_>>();
        if args.len() > 1 && (args.contains(&"--help".to_string()) || args.contains(&"-h".to_string())) {
            eprintln!("Usage: template-worker [worker_id]");
            exit(1);
        }
        let worker_id = args.get(1)
        .expect("Worker ID must be provided as the first argument")
        .parse().expect("Worker ID must be a valid number");

        let tokio_threads = std::env::var("TOKIO_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::TOKIO_THREADS);
        let worker_debug = std::env::var("WORKER_DEBUG")
            .ok()
            .and_then(|s| Some(s.to_lowercase() == "true" || s == "1"))
            .unwrap_or(Self::WORKER_DEBUG);
        Self { tokio_threads, worker_debug, worker_id }
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

    let (http, stratum, current_user) = setup_discord().await;

    let (meso_client, meso_client_stream) = MesophyllClient::new(worker_id)
        .await
        .expect("Failed to create Mesophyll client");

    let worker_state = WorkerState::new(
        http.clone(),
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
