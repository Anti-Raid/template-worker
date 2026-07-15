use dapi::{ChannelId, UserId};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::sync::LazyLock;
use crate::Error;

/// Global config object
pub static CONFIG: LazyLock<Config> =
    LazyLock::new(|| Config::load().expect("Failed to load config"));

#[derive(Serialize, Deserialize)]
pub struct Config {
    // Discord core
    pub nirn_token: String,
    pub client_id: UserId,
    pub client_secret: String,
    pub allowed_redirects: Vec<String>,

    // meta
    pub postgres_url: String,
    pub proxy: String,
    pub support_server_invite: String,
    pub default_error_channel: ChannelId,
    pub mesophyll_token: String,
    pub blob_token: String,
    pub stratum_server: String,
    pub stratum_grpc_access_key: String,

    // sites
    pub api: String,
    pub frontend: String,
    pub docs: String,

    // addresses
    pub template_worker_bind_addr: String,
    pub mesophyll_server_bind_addr: String,

    // misc
    pub worker_path: PathBuf,

    #[serde(skip)]
    /// Setup by load() for statistics
    pub start_time: chrono::DateTime<chrono::Utc>,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        // Open config.yaml from parent directory
        let file = File::open("config.json");

        match file {
            Ok(file) => {
                // Parse config.yaml
                let mut cfg: Config = serde_json::from_reader(file)?;

                cfg.start_time = chrono::Utc::now();

                // Return config
                Ok(cfg)
            }
            Err(e) => {
                // Print error
                println!("config.json could not be loaded: {}", e);

                // Exit
                std::process::exit(1);
            }
        }
    }
}
