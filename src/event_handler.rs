use crate::dispatch::discord_event_dispatch;
use crate::expiry_tasks::tasks;
use async_trait::async_trait;
use serenity::all::Framework;
use serenity::gateway::client::Context;

static ONCE: std::sync::Once = std::sync::Once::new();

pub struct EventFramework {}

#[async_trait]
impl Framework for EventFramework {
    async fn dispatch(&self, ctx: &Context, event: &serenity::all::FullEvent) {
        if let serenity::all::FullEvent::Ready { .. } = event {
            ONCE.call_once(|| {
                let ctx1 = ctx.clone();
                let data1 = ctx.data::<silverpelt::data::Data>();

                tokio::task::spawn(async move {
                    log::info!("Starting RPC server");

                    let rpc_server = crate::http::create(data1, &ctx1);

                    let opts = rust_rpc_server::CreateRpcServerOptions {
                        bind: rust_rpc_server::CreateRpcServerBind::Address(format!(
                            "{}:{}",
                            config::CONFIG.base_ports.template_worker_addr,
                            config::CONFIG.base_ports.template_worker_port
                        )),
                    };

                    rust_rpc_server::start_rpc_server(opts, rpc_server).await;
                });

                let ctx2 = ctx.clone();

                tokio::task::spawn(async move {
                    log::info!("Calling on_startup");
                    crate::on_startup::on_startup(ctx2)
                        .await
                        .expect("Failed to call on_startup");
                });

                let ctx3 = ctx.clone();
                tokio::task::spawn(async move {
                    log::info!("Starting up tasks");

                    tokio::task::spawn(async move {
                        botox::taskman::start_all_tasks(tasks(), ctx3).await;
                    });
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
