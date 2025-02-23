use crate::config::CONFIG;
use crate::dispatch::{discord_event_dispatch, dispatch, parse_event};
use crate::expiry_tasks::tasks;
use crate::templatingrt::cache::{get_all_guild_templates, get_all_guilds};
use antiraid_types::ar_event::AntiraidEvent;
use async_trait::async_trait;
use serenity::all::Framework;
use serenity::gateway::client::Context;

static ONCE: std::sync::Once = std::sync::Once::new();

pub struct EventFramework {}

#[async_trait]
impl Framework for EventFramework {
    async fn init(&mut self, client: &serenity::all::Client) {
        // Set up the shard messenger
        crate::serenitystore::setup_shard_messenger(client).await;
    }

    async fn dispatch(&self, ctx: &Context, event: &serenity::all::FullEvent) {
        if let serenity::all::FullEvent::Ready { .. } = event {
            crate::serenitystore::update_shard_messengers().await;
            ONCE.call_once(|| {
                let ctx1 = ctx.clone();
                let data1 = ctx.data::<silverpelt::data::Data>();

                tokio::task::spawn(async move {
                    log::info!("Starting RPC server");

                    let rpc_server = crate::http::create(data1, &ctx1);

                    let opts = rust_rpc_server::CreateRpcServerOptions {
                        bind: rust_rpc_server::CreateRpcServerBind::Address(format!(
                            "{}:{}",
                            CONFIG.base_ports.template_worker_bind_addr,
                            CONFIG.base_ports.template_worker_port
                        )),
                    };

                    rust_rpc_server::start_rpc_server(opts, rpc_server).await;
                });

                let ctx2 = ctx.clone();
                tokio::task::spawn(async move {
                    log::info!("Starting up tasks");

                    tokio::task::spawn(async move {
                        botox::taskman::start_all_tasks(tasks(), ctx2).await;
                    });
                });

                // Fire OnStartup event to all templates
                let ctx3 = ctx.clone();
                tokio::task::spawn(async move {
                    log::info!("Firing OnStartup event to all templates");

                    for guild in get_all_guilds() {
                        let ctx = ctx3.clone();
                        let data = ctx.data::<silverpelt::data::Data>();
                        tokio::task::spawn(async move {
                            let Some(templates) = get_all_guild_templates(guild).await else {
                                return;
                            };
                            let templates = templates.iter().map(|t| t.name.clone()).collect();
                            let create_event =
                                match parse_event(&AntiraidEvent::OnStartup(templates)) {
                                    Ok(e) => e,
                                    Err(e) => {
                                        log::error!("Error parsing event: {:?}", e);
                                        return;
                                    }
                                };

                            match dispatch(&ctx, &data, create_event, guild).await {
                                Ok(_) => {}
                                Err(e) => {
                                    log::error!("Error dispatching event: {:?}", e);
                                }
                            }
                        });
                    }
                });
            });
        }

        match discord_event_dispatch(event, ctx).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("Error dispatching event: {:?}", e);
            }
        }
    }
}
