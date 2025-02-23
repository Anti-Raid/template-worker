use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};
use serenity::all::UserId;
use silverpelt::objectstore::ObjectStore;
use std::fs::File;
use std::sync::LazyLock;

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, ValueEnum, Clone, Copy)]
pub enum VmDistributionStrategy {
    /// Use a thread pool that each stores a set of VMs where each guild is assigned a VM on the threadpool
    ThreadPool,
    /// Use a dedicated thread for each VM where each guild is assigned a VM (on a dedicated thread)
    ThreadPerGuild,
}

/// Command line arguments
#[derive(Parser, Debug, Clone)]
pub struct CmdArgs {
    /// Shard IDs to start. Mutually exclusive with `shard_count` and neither passed [default is to autoshard]
    #[clap(long)]
    pub shards: Option<Vec<u16>>,

    /// Number of shards to start. Mutually exclusive with `shards` and neither passed [default is to autoshard]
    #[clap(long)]
    pub shard_count: Option<u16>,

    /// Max connections that should be made to the database
    #[clap(long, default_value = "7")]
    pub max_db_connections: u32,

    /// Distribution strategy for VMs
    #[clap(long, default_value = "thread-pool")]
    pub vm_distribution_strategy: VmDistributionStrategy,
}

/// Global internal config object
pub static CMD_ARGS: LazyLock<CmdArgs> = LazyLock::new(CmdArgs::parse);

/// Global config object
pub static CONFIG: LazyLock<Config> =
    LazyLock::new(|| Config::load().expect("Failed to load config"));

#[derive(Serialize, Deserialize, Default)]
pub struct DiscordAuth {
    pub token: String,
    pub client_id: String,
    pub client_secret: String,
    pub root_users: Vec<UserId>,
}

// Object storage code
#[derive(Serialize, Deserialize)]
pub enum ObjectStorageType {
    #[serde(rename = "s3-like")]
    S3Like,
    #[serde(rename = "local")]
    Local,
}

#[derive(Serialize, Deserialize)]
pub struct ObjectStorage {
    #[serde(rename = "type")]
    pub object_storage_type: ObjectStorageType,
    pub path: String,
    pub endpoint: Option<String>,
    pub secure: Option<bool>,
    pub cdn_secure: Option<bool>,
    pub cdn_endpoint: String,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

impl ObjectStorage {
    pub fn build(&self) -> Result<ObjectStore, silverpelt::Error> {
        match self.object_storage_type {
            ObjectStorageType::S3Like => {
                let access_key = self.access_key.as_ref().ok_or("Missing access key")?;
                let secret_key = self.secret_key.as_ref().ok_or("Missing secret key")?;
                let endpoint = self.endpoint.as_ref().ok_or("Missing endpoint")?;

                let endpoint_url = format!(
                    "{}://{}",
                    if self.secure.unwrap_or(false) {
                        "https"
                    } else {
                        "http"
                    },
                    endpoint
                );

                ObjectStore::new_s3(
                    "antiraid.rust".to_string(),
                    endpoint_url,
                    access_key.to_string(),
                    secret_key.to_string(),
                )
            }
            ObjectStorageType::Local => Ok(ObjectStore::new_local(self.path.clone())),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
pub struct Meta {
    pub postgres_url: String,
    pub proxy: String,
    pub support_server_invite: String,
    pub sandwich_http_api: String,
}

#[derive(Serialize, Deserialize)]
pub struct Sites {
    pub api: String,
    pub frontend: String,
    pub docs: String,
}

#[derive(Serialize, Deserialize)]
pub struct Servers {
    pub main: serenity::all::GuildId,
}

#[derive(Serialize, Deserialize)]
pub struct BasePorts {
    pub jobserver: u16,
    pub bot: u16,
    pub jobserver_base_addr: String,
    pub jobserver_bind_addr: String,
    pub bot_base_addr: String,
    pub bot_bind_addr: String,
    pub template_worker_bind_addr: String,
    pub template_worker_addr: String,
    pub template_worker_port: u16,
}

#[derive(Serialize, Deserialize)]
pub struct Config {
    pub discord_auth: DiscordAuth,
    pub meta: Meta,
    pub sites: Sites,
    pub servers: Servers,
    pub object_storage: ObjectStorage,
    pub base_ports: BasePorts,

    #[serde(skip)]
    /// Setup by load() for statistics
    pub start_time: i64,
}

impl Config {
    pub fn load() -> Result<Self, Error> {
        // Open config.yaml from parent directory
        let file = File::open("config.yaml");

        match file {
            Ok(file) => {
                // Parse config.yaml
                let mut cfg: Config = serde_yaml::from_reader(file)?;

                cfg.start_time = chrono::Utc::now().timestamp();

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
