use dapi::dhttp::HttpCall;
use tw::master::register;
use tw::setup_discord;
use log::info;
use std::io::Write;

/// Command line arguments
#[derive(Debug, Clone)]
struct CmdArgs {
    /// How many tokio threads to use
    pub tokio_threads: usize,
}

impl CmdArgs {
    const TOKIO_THREADS: usize = 3;
    pub fn parse() -> Self {
        let tokio_threads = std::env::var("TOKIO_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::TOKIO_THREADS);
        Self { tokio_threads }
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
        main_impl().await;
    });
}

async fn main_impl() {
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

    let stratum = setup_discord().await;

    info!("Getting registration data from builtins");

    let data = &*register::REGISTER;

    println!("Register data: {:?}", data);

    stratum.discord_http().call_fire(HttpCall::CreateGlobalCommands { 
        application_id: stratum.discord_http().app_id(),
        map: serde_json::to_vec(&data.commands).expect("Failed to create global commands"),
    })
    .await
    .expect("Failed to register commands");
}
