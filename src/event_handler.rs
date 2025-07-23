use crate::config::CONFIG;
use crate::dispatch::{discord_event_dispatch, dispatch, parse_event};
use crate::http::init::{start_rpc_server, CreateRpcServerBind, CreateRpcServerOptions};
use crate::templatingrt::cache::{get_all_guild_templates, get_all_guilds_with_templates};
use antiraid_types::ar_event::AntiraidEvent;
use async_trait::async_trait;
use serenity::all::{EventHandler, IEvent};
use serenity::gateway::client::Context;

static ONCE: std::sync::Once = std::sync::Once::new();

pub struct EventFramework {}

#[async_trait]
impl EventHandler for EventFramework {
    async fn dispatch(&self, ctx: &Context, event: &IEvent) {
        if event.ty == "GUILD_CREATE" {
            // Ignore guild create events
            return;
        }

        if event.ty == "READY" {
            ONCE.call_once(|| {
                let ctx1 = ctx.clone();
                let data1 = ctx.data::<crate::data::Data>();

                tokio::task::spawn(async move {
                    log::info!("Starting RPC server");

                    let rpc_server = crate::http::server::create(data1, &ctx1);

                    let opts = CreateRpcServerOptions {
                        bind: CreateRpcServerBind::Address(format!(
                            "{}:{}",
                            CONFIG.base_ports.template_worker_bind_addr,
                            CONFIG.base_ports.template_worker_port
                        )),
                    };

                    start_rpc_server(opts, rpc_server).await;
                });

                // Fire OnStartup event to all templates
                let ctx2 = ctx.clone();
                tokio::task::spawn(async move {
                    log::info!("Firing OnStartup event to all templates");

                    for guild in get_all_guilds_with_templates() {
                        let ctx = ctx2.clone();
                        let data = ctx.data::<crate::data::Data>();
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

                // Start up the key expiry task
                let ctx3 = ctx.clone();
                tokio::task::spawn(async move {
                    crate::expiry_tasks::key_expiry_task(ctx3).await;
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
