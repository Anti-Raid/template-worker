pub mod config;
pub mod mesophyll;
pub mod worker;
pub mod migrations;
pub mod geese;
pub mod master;

use std::{sync::Arc, time::Duration};
use log::{debug, error};
use serenity::all::{ApplicationId, CurrentUser, Http, HttpBuilder};
use crate::geese::stratum::Stratum;

pub use crate::config::CONFIG;

// This is constant and should be copy pasted
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// Helper method to setup discord related state in a tw binary
pub async fn setup_discord() -> (Arc<Http>, Stratum, CurrentUser) {
    let proxy_url = CONFIG.meta.proxy.clone();

    debug!("Proxy URL: {}", proxy_url);

    let token = serenity::all::SecretString::new(CONFIG.discord_auth.token.clone().into());
    let http = Arc::new(HttpBuilder::new(token.clone()).proxy(proxy_url).build());

    let stratum = Stratum::new(http.clone()).await.expect("Failed to connect to stratum");

    let current_user = loop {
        match stratum.current_user().await {
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

    debug!("Current user: {} ({})", current_user.name, current_user.id);
    http.set_application_id(ApplicationId::new(current_user.id.get()));

    (http, stratum, current_user)
}
