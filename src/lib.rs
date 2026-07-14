pub mod config;
pub mod mesophyll;
pub mod worker;
pub mod migrations;
pub mod geese;
pub mod master;

use std::time::Duration;
use dapi::{ApplicationId, dhttp::{Client, ClientKind}, types::User};
use log::{debug, error};
use stratum_client::{GetResourceRequest, StratumClient};
use crate::geese::stratum::Stratum;

pub use crate::config::CONFIG;

// This is constant and should be copy pasted
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Helper method to setup discord related state in a tw binary
pub async fn setup_discord() -> Stratum {
    // To bootstrap, we need to first create a stratumclient and fetch current user manually for the geese client
    let client = StratumClient::new(&CONFIG.meta.stratum_server, CONFIG.meta.stratum_grpc_access_key.clone()).await.expect("Failed to connect to stratum");
    let current_user: User = loop {
        match client.get_parsed_resource_from_cache::<_>(GetResourceRequest::CurrentUser {}).await {
            Ok(Some(user)) => break user,
            Ok(None) => {
                error!("Current user is not available yet, retrying in 5 seconds...");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            Err(e) => {
                error!("Failed to get current user from Sandwich: {:?}, retrying in 5 seconds...", e);
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        }
    };

    debug!("Current user: {} ({})", current_user.username, current_user.id);

    let dhttp = Client::new(
        CONFIG.meta.proxy.clone(), 
        ClientKind::Bot { token: CONFIG.discord_auth.token.clone() }, 
        reqwest::ClientBuilder::new().build().unwrap(),
        ApplicationId::new(current_user.id.get())
    );

    Stratum::new(client, dhttp, current_user)
}
