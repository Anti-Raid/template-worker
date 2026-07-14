use dapi::{ChannelId, GuildId, UserId};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::path::PathBuf;
use std::sync::LazyLock;
use crate::Error;

/// Global config object
pub static CONFIG: LazyLock<Config> =
    LazyLock::new(|| Config::load().expect("Failed to load config"));

#[derive(Serialize, Deserialize, Default)]
pub struct DiscordAuth {
    pub token: String,
    pub client_id: UserId,
    pub client_secret: String,
    pub root_users: Vec<UserId>,
    pub allowed_redirects: Vec<String>
}

#[derive(Serialize, Deserialize, Default)]
pub struct Meta {
    pub postgres_url: String,
    pub proxy: String,
    pub support_server_invite: String,
    pub default_error_channel: ChannelId,
    pub mesophyll_token: String,
    pub blob_token: String,
    pub stratum_server: String,
    pub stratum_grpc_access_key: String
}

#[derive(Serialize, Deserialize)]
pub struct Sites {
    pub api: String,
    pub frontend: String,
    pub docs: String,
}

#[derive(Serialize, Deserialize)]
pub struct Servers {
    pub main: GuildId,
}

#[derive(Serialize, Deserialize)]
pub struct Addrs {
    pub template_worker: String,
    pub mesophyll_server: String,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub discord_auth: DiscordAuth,
    pub meta: Meta,
    pub sites: Sites,
    pub servers: Servers,
    pub addrs: Addrs,
    pub worker_path: PathBuf,

    #[serde(skip)]
    /// Setup by load() for statistics
    pub start_time: chrono::DateTime<chrono::Utc>,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        // Open config.yaml from parent directory
        let file = File::open("config.yaml");

        match file {
            Ok(file) => {
                // Parse config.yaml
                let mut cfg: Config = serde_saphyr::from_reader(file)?;

                cfg.start_time = chrono::Utc::now();

                // Return config
                Ok(cfg)
            }
            Err(e) => {
                // Print error
                println!("config.yaml could not be loaded: {}", e);

                // Exit
                std::process::exit(1);
            }
        }
    }
}
