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
                let ctx = ctx.clone();
                let data = ctx.data::<silverpelt::data::Data>().clone();
                tokio::task::spawn(async move {
                    log::info!("Starting RPC server");

                    let rpc_server = crate::http::create(data.clone(), &ctx);

                    let opts = rust_rpc_server::CreateRpcServerOptions {
                        bind: rust_rpc_server::CreateRpcServerBind::Address(format!(
                            "{}:{}",
                            config::CONFIG.base_ports.template_worker_addr,
                            config::CONFIG.base_ports.template_worker_port
                        )),
                    };

                    tokio::task::spawn(async move {
                        rust_rpc_server::start_rpc_server(opts, rpc_server).await;
                        panic!("RPC server exited unexpectedly");
                    });

                    log::info!("Calling on_startup");
                    crate::startup::on_startup(ctx.clone())
                        .await
                        .expect("Failed to call on_startup");
                });
            });
        }

        match crate::dispatch::discord_event_dispatch(event, ctx).await {
            Ok(_) => {}
            Err(e) => {
                log::error!("Error dispatching event: {:?}", e);
            }
        }
    }
}
