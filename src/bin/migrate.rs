use tw::config::CONFIG;
use tw::migrations::apply_migrations;
use sqlx::postgres::PgPoolOptions;
use std::io::Write;

/// Command line arguments
#[derive(Debug, Clone)]
struct CmdArgs {
    /// Max connections that should be made to the database
    pub max_db_connections: u32,

    /// How many tokio threads to use
    pub tokio_threads: usize,
}

impl CmdArgs {
    const MAX_DB_CONNECTIONS: u32 = 7;
    const TOKIO_THREADS: usize = 10;
    pub fn parse() -> Self {
        let max_db_connections = std::env::var("MAX_DB_CONNECTIONS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::MAX_DB_CONNECTIONS);
        let tokio_threads = std::env::var("TOKIO_THREADS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(Self::TOKIO_THREADS);
        Self { max_db_connections, tokio_threads }
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

    let pg_pool = PgPoolOptions::new()
        .max_connections(args.max_db_connections)
        .connect(&CONFIG.postgres_url)
        .await
        .expect("Could not initialize connection");

    apply_migrations(pg_pool).await.expect("Failed to apply migrations");

}