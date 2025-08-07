use crate::config::CONFIG;
use crate::dispatch::discord_event_dispatch;
use crate::api::init::{start_rpc_server, CreateRpcServerBind, CreateRpcServerOptions};
use async_trait::async_trait;
use serenity::all::{EventHandler, IEvent};
use serenity::gateway::client::Context;

static ONCE: std::sync::Once = std::sync::Once::new();

pub struct EventFramework {}

#[async_trait]
impl EventHandler for EventFramework {
    async fn dispatch(&self, ctx: &Context, event: IEvent) {
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

                    let rpc_server = crate::api::server::create(data1, &ctx1);

                    let opts = CreateRpcServerOptions {
                        bind: CreateRpcServerBind::Address(format!(
                            "{}:{}",
                            CONFIG.base_ports.template_worker_bind_addr,
                            CONFIG.base_ports.template_worker_port
                        )),
                    };

                    start_rpc_server(opts, rpc_server).await;
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
