use tw::master::shell;
use tw::mesophyll::client::MesophyllShellClient;
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

    shell::init_shell(MesophyllShellClient::new().await.expect("failed to create meso shell client"));
}
